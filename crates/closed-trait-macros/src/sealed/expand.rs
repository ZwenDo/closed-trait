use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    GenericArgument, GenericParam, Ident, ItemTrait, Lifetime, Path, PathArguments, parse_quote,
};

use super::input::{Input, SealedType};
use crate::util::{argument, lifetimes, mentions, name_of, render};

pub(crate) fn expand(input: Input) -> TokenStream {
    let Input { mut item, types } = input;

    // Private to the module the attribute was written in, so nothing outside
    // can name `Sealed`, let alone implement it.
    // The trait's name verbatim, not snake_cased: `Shape` and `SHAPE` would
    // otherwise land on the same module.
    let module = format_ident!("__sealed_{}", item.ident);

    // The marker carries the trait's type and const parameters, so the seal is
    // as precise as the list: `Plain: Store<i32>` permits `Store<i32>` and
    // nothing else. Lifetimes stay off it -- two impls differing only in one
    // overlap, so they could never tell two entries apart.
    let marker_params: Vec<TokenStream> = item
        .generics
        .params
        .iter()
        .filter_map(marker_param)
        .collect();
    let marker_args: Vec<TokenStream> = item
        .generics
        .params
        .iter()
        .filter(|param| !matches!(param, GenericParam::Lifetime(_)))
        .map(argument)
        .collect();
    let marker_declarations = (!marker_params.is_empty()).then(|| quote!(<#(#marker_params),*>));
    let marker_arguments = (!marker_args.is_empty()).then(|| quote!(<#(#marker_args),*>));

    item.supertraits
        .push(parse_quote!(#module::Sealed #marker_arguments));

    let assertion = assertion(&item, &types);
    // Both impls live inside the module, which is the point: `Sealed` is
    // reachable from the enclosing scope, but `Unforgeable` is not, so the only
    // place both can be satisfied is here.
    //
    // Both are per entry and both carry the arguments, which is what keeps the
    // seal unforgeable *per instantiation*: a hand-written `Sealed<f64>` beside
    // the trait would need `Unforgeable<f64>`, and only this module can write
    // one. Parameterising just `Sealed` would leave an already-sealed type open
    // at every other instantiation.
    //
    // Deduplicated on the whole impl header, since an entry may legitimately be
    // repeated at different arguments but not at the same ones.
    let mut written = Vec::new();
    let impls: Vec<TokenStream> = types
        .iter()
        .filter_map(|entry| {
            let generics = declarations(&params(entry, &item));
            let arguments = pinned(entry, &item);
            let arguments = (!arguments.is_empty()).then(|| quote!(<#(#arguments),*>));
            let ty = &entry.ty;

            let already = render(&quote!(#generics #arguments #ty));
            if written.contains(&already) {
                return None;
            }
            written.push(already);

            Some(quote! {
                impl #generics Sealed #arguments for #ty {}
                impl #generics unforgeable::Unforgeable #arguments for #ty {}
            })
        })
        .collect();

    // Without this, a rejected implementor is told only that it does not
    // implement a `Sealed` trait it has never heard of and cannot name.
    let ident = &item.ident;
    let message = format!("`{{Self}}` cannot implement `{ident}`");
    let label = format!("not permitted to implement `{ident}` here");
    let note = format!(
        "only the types listed in `#[sealed(..)]` on `{ident}` may implement it, and only at the \
         instantiations listed there"
    );

    let forged =
        format!("`{{Self}}` cannot be given `{ident}`'s seal from outside the macro that wrote it");
    let forged_note =
        format!("add the type to `#[sealed(..)]` on `{ident}` instead of implementing this");

    quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        mod #module {
            // The listed types are written in the enclosing scope, so their
            // names have to be reachable from in here.
            #[allow(unused_imports)]
            use super::*;

            #[diagnostic::on_unimplemented(
                message = #message,
                label = #label,
                note = #note,
            )]
            pub trait Sealed #marker_declarations: unforgeable::Unforgeable #marker_arguments {}

            // Private to this module, so `Sealed` cannot be satisfied anywhere
            // else -- not even by code sitting beside the sealed trait, which
            // can name `Sealed` itself.
            mod unforgeable {
                #[diagnostic::on_unimplemented(
                    message = #forged,
                    note = #forged_note,
                )]
                pub trait Unforgeable #marker_declarations {}
            }

            #(#impls)*
        }

        #assertion

        #item
    }
}

/// The parameters an entry's generated items have to declare.
///
/// Three sources, in order: the entry's own `for<..>` binder, the trait's
/// parameters that the type mentions, and any remaining named lifetimes.
///
/// A bare name is a parameter only if one of the first two declares it, so
/// `Boxed<T>` means every `Boxed` under `trait Store<T>` or after `for<T>`, and
/// whatever concrete `T` is in scope otherwise.
///
/// The third source only ever sees `'static` and `'_`: a named lifetime that
/// neither the trait nor a binder declares is refused when the input is
/// parsed. Accepting it would make `Bar<'a>` mean the trait's lifetime or
/// every lifetime depending on what the trait happened to call its parameter,
/// so renaming that parameter would change what is sealed.
fn params(entry: &SealedType, item: &ItemTrait) -> Vec<(String, TokenStream)> {
    let ty = &entry.ty;
    let named = lifetimes(ty);
    let mut params: Vec<(String, TokenStream)> = Vec::new();
    let declared = |params: &[(String, TokenStream)], name: &str| {
        params.iter().any(|(known, _)| known == name)
    };

    if let Some(binder) = &entry.binder {
        for param in &binder.params {
            params.push((name_of(param), quote!(#param)));
        }
    }

    for param in &item.generics.params {
        let name = name_of(param);
        if declared(&params, &name) {
            continue;
        }
        let used = match param {
            GenericParam::Lifetime(param) => named.contains(&param.lifetime.ident),
            GenericParam::Type(_) | GenericParam::Const(_) => mentions(ty, &name),
        };
        if used {
            params.push((name, quote!(#param)));
        }
    }

    for lifetime in named {
        let name = lifetime.to_string();
        if declared(&params, &name) {
            continue;
        }
        let lifetime = Lifetime::new(&format!("'{lifetime}"), lifetime.span());
        params.push((name, quote!(#lifetime)));
    }

    params
}

fn declarations(params: &[(String, TokenStream)]) -> Option<TokenStream> {
    let declarations = params.iter().map(|(_, tokens)| tokens);
    (!params.is_empty()).then(|| quote!(<#(#declarations),*>))
}

/// Checks that every listed type really does implement the trait, so the list
/// stays accurate rather than merely permissive.
///
/// Every entry is checked. The trait's type and const parameters have to be
/// supplied for it, which the entry guarantees by either naming them itself, as
/// `Boxed<T>` does under `trait Store<T>`, or annotating its instantiation, as
/// in `Plain: Store<i32>`. An entry that does neither is refused when the input
/// is parsed, so there is nothing to skip here.
///
/// Lifetimes never need supplying, and not merely because inference usually
/// copes: a type cannot implement the same trait at two different lifetimes,
/// since two such impls overlap and coherence rejects them. There is never more
/// than one candidate, so the turbofish leaves them out and lets inference
/// settle it.
fn assertion(item: &ItemTrait, types: &[SealedType]) -> Option<TokenStream> {
    let ident = &item.ident;
    let trait_params = &item.generics.params;
    let names = item.generics.params.iter().map(argument);
    let trait_arguments = (!trait_params.is_empty()).then(|| quote!(<#(#names),*>));
    let trait_params = (!trait_params.is_empty()).then(|| quote!(#trait_params,));

    // Type and const parameters have to be supplied by name; lifetimes are
    // always inferable and so never appear in the turbofish.
    let supplied: Vec<String> = item
        .generics
        .params
        .iter()
        .filter(|param| !matches!(param, GenericParam::Lifetime(_)))
        .map(name_of)
        .collect();

    // One function per entry, so a quantified type has somewhere to declare its
    // parameters. Taking the type by reference brings its own well-formedness
    // in as an implied bound, which is what lets `Pair<'a, T>` be checked
    // without the caller having to spell out `T: 'a`; a reference also keeps
    // unsized entries like `str` usable. They are never called, hence the
    // `allow`.
    let checks: Vec<TokenStream> = types
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let params = params(entry, item);
            let arguments = match &entry.instantiation {
                Some(path) => instantiation(path),
                // Without an annotation the type names the trait's parameters
                // itself, which parsing has already insisted on.
                None => supplied
                    .iter()
                    .map(|name| {
                        let name = Ident::new(name, Span::call_site());
                        quote!(#name)
                    })
                    .collect(),
            };

            let check = format_ident!("check_{index}");
            let declarations = declarations(&params);
            let ty = &entry.ty;
            quote! {
                fn #check #declarations(value: &#ty) {
                    assert::<#(#arguments,)* _>(value);
                }
            }
        })
        .collect();

    if checks.is_empty() {
        return None;
    }

    Some(quote! {
        #[allow(dead_code)]
        const _: () = {
            fn assert<#trait_params S: #ident #trait_arguments + ?Sized>(_: &S) {}
            #(#checks)*
        };
    })
}

/// A trait parameter as the marker declares it, bounds dropped: the marker is
/// empty, so a bound could only fail to resolve. Lifetimes are left out.
fn marker_param(param: &GenericParam) -> Option<TokenStream> {
    match param {
        GenericParam::Lifetime(_) => None,
        GenericParam::Type(param) => {
            let ident = &param.ident;
            Some(quote!(#ident))
        }
        GenericParam::Const(param) => {
            let ident = &param.ident;
            let ty = &param.ty;
            Some(quote!(const #ident: #ty))
        }
    }
}

/// The trait's type and const parameters as one entry supplies them.
///
/// Either the entry annotates its instantiation, or the type names them itself,
/// which parsing has already insisted on.
fn pinned(entry: &SealedType, item: &ItemTrait) -> Vec<TokenStream> {
    match &entry.instantiation {
        Some(path) => instantiation(path),
        None => item
            .generics
            .params
            .iter()
            .filter(|param| !matches!(param, GenericParam::Lifetime(_)))
            .map(argument)
            .collect(),
    }
}

/// The arguments of an annotation like `Store<i32>`, ready for a turbofish.
fn instantiation(path: &Path) -> Vec<TokenStream> {
    let Some(segment) = path.segments.last() else {
        return Vec::new();
    };
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Vec::new();
    };
    // Lifetimes stay out, exactly as they do when the entry names the trait's
    // parameters itself. `assert` takes them late bound, so naming one in the
    // turbofish is an error in waiting -- and there is nothing to disambiguate
    // anyway, two impls differing only in a lifetime being rejected by
    // coherence.
    arguments
        .args
        .iter()
        .filter(|arg| !matches!(arg, GenericArgument::Lifetime(_)))
        .map(|arg| quote!(#arg))
        .collect()
}
