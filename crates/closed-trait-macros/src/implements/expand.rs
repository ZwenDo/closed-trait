use crate::implements::input::{Input, mentions};
use crate::util::render;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{FnArg, GenericParam, Ident, Pat, Type};

pub(crate) fn expand(input: Input) -> TokenStream {
    let Input {
        item,
        macro_vis,
        macro_name,
        subject: subject_generic,
        shape: probe_shape,
        bounds: subject_bounds,
        carried_where,
    } = &input;

    let signature = &item.sig;
    let name = &signature.ident;
    let asyncness = &signature.asyncness;
    let output = &signature.output;

    let mut inputs = signature.inputs.iter();
    let subject = match inputs.next() {
        Some(FnArg::Typed(pat)) => pat,
        _ => unreachable!("`Input::parse` refused a signature without a first parameter"),
    };
    let rest: Vec<_> = inputs.collect();

    let carried: Vec<_> = signature
        .generics
        .params
        .iter()
        .filter(|param| !matches!(param, GenericParam::Type(ty) if ty.ident == *subject_generic))
        .collect();
    // A parameter is settled either by an argument, whose type mentions it, or by
    // the call site, when the result's type does: what this hands back keeps its
    // return type open until it is called. A parameter the signature never mentions
    // is settled by neither, and has to be named at the invocation.
    let mentioned = {
        let types: Vec<TokenStream> = signature
            .inputs
            .iter()
            .map(|argument| match argument {
                FnArg::Typed(pat) => {
                    let ty = &pat.ty;
                    quote!(#ty)
                }
                FnArg::Receiver(_) => unreachable!("only the first parameter may be a receiver"),
            })
            .collect();
        quote!(#(#types)* #output)
    };
    let mut carried_args: Vec<&Ident> = Vec::new();
    let mut required: Vec<&Ident> = Vec::new();
    for param in &signature.generics.params {
        let ident = match param {
            GenericParam::Lifetime(_) => continue,
            GenericParam::Type(ty) => &ty.ident,
            GenericParam::Const(constant) => &constant.ident,
        };
        if ident == subject_generic {
            continue;
        }
        carried_args.push(ident);
        if mentions(mentioned.clone(), ident) {
            continue;
        }
        // A const argument is a literal or a braced expression, and a `ty` fragment
        // matches neither, so the list cannot carry one. Left out of what is written
        // as suppliable rather than refused here: rustc has the last word on whether
        // anything is missing, and says so against the call.
        if matches!(param, GenericParam::Const(_)) {
            continue;
        }
        required.push(ident);
    }

    // Supplying one parameter means supplying them all, so the list the macro takes
    // is every parameter `get` declares, with `_` wherever inference still copes.
    let placeholders: Vec<String> = carried_args
        .iter()
        .map(|ident| match required.contains(ident) {
            true => ident.to_string(),
            false => "_".to_string(),
        })
        .collect();
    let placeholders = placeholders.join(", ");

    // Taken as tokens rather than as a list of types, and handed to the turbofish as
    // written. A `ty` fragment matches no const argument, so `3` and `{ 2 + 1 }` would
    // be refused by the rule rather than by rustc, and only a const already bound to a
    // name would pass. Whatever the tokens are, what comes of them is a turbofish
    // rustc reports against the call: too long, too short, or unwanted.
    let matcher = quote!($value:expr $(; $($given:tt)*)?);
    let turbofish = quote!($(::<$($given)*>)?);

    // `get`'s own parameters, handed on by name. Without this the call inside it is
    // a second place with nothing to infer from, which reports against the function
    // rather than against the invocation.
    let forwarded: Vec<&Ident> = signature
        .generics
        .params
        .iter()
        .filter_map(|param| match param {
            GenericParam::Lifetime(_) => None,
            GenericParam::Type(ty) => Some(&ty.ident),
            GenericParam::Const(constant) => Some(&constant.ident),
        })
        .collect();
    let forwarded = (!forwarded.is_empty()).then(|| quote!(::<#(#forwarded),*>));
    // `new` only needs the lifetimes, since the shape may name them; declaring
    // the type parameters there too would leave them unconstrained.
    let lifetimes: Vec<_> = signature
        .generics
        .params
        .iter()
        .filter(|param| matches!(param, GenericParam::Lifetime(_)))
        .collect();
    let lifetimes = (!lifetimes.is_empty()).then(|| quote!(<#(#lifetimes),*>));
    let carried = (!carried.is_empty()).then(|| quote!(<#(#carried),*>));

    let names: Vec<TokenStream> = rest
        .iter()
        .enumerate()
        .map(|(index, argument)| match argument {
            FnArg::Typed(pat) => match &*pat.pat {
                Pat::Ident(ident) => {
                    let ident = &ident.ident;
                    quote!(#ident)
                }
                _ => {
                    let generated = format_ident!("arg{index}");
                    quote!(#generated)
                }
            },
            FnArg::Receiver(_) => unreachable!("only the first parameter may be a receiver"),
        })
        .collect();
    let types: Vec<&Type> = rest
        .iter()
        .map(|argument| match argument {
            FnArg::Typed(pat) => &*pat.ty,
            FnArg::Receiver(_) => unreachable!("only the first parameter may be a receiver"),
        })
        .collect();

    let ret = match output {
        syn::ReturnType::Default => quote!(()),
        syn::ReturnType::Type(_, ty) => quote!(#ty),
    };
    let handed_back = match asyncness {
        Some(_) => quote!(impl ::core::future::Future<Output = #ret>),
        None => ret.clone(),
    };
    let unreached = match asyncness {
        Some(_) => quote!(async { ::core::unreachable!() }),
        None => quote!(::core::unreachable!()),
    };

    // The function is emitted as written, so it is type-checked once where it was
    // declared and an editor has a real item to resolve against. The macro calls it
    // rather than carrying a copy of its body, which is why it has to be in scope
    // wherever the macro is used.

    // The function's own name for the subject, so what is written about the call has
    // the shape of the signature rather than something to translate.
    let subject_name = match &*subject.pat {
        Pat::Ident(ident) => ident.ident.to_string(),
        _ => "value".to_string(),
    };

    let rule = quote! {
        (#matcher) => {
            {
                use ::core::marker::PhantomData;
                use ::core::option::Option;

                #[allow(dead_code)]
                struct Probe<#subject_generic>(PhantomData<fn() -> #subject_generic>);

                // Manual rather than derived: `derive` would bound this on the
                // subject's type being `Copy`, while the field is `Copy` whatever
                // that type is. Copying is what lets `get` take `self` by value —
                // so the result never borrows the probe — while the closure still
                // only reads its capture, and is therefore an `Fn`.
                impl<#subject_generic> Clone for Probe<#subject_generic> {
                    fn clone(&self) -> Self { *self }
                }
                impl<#subject_generic> Copy for Probe<#subject_generic> {}

                impl<#subject_generic> Probe<#subject_generic> {
                    #[allow(dead_code)]
                    fn new #lifetimes (_: &#probe_shape) -> Self {
                        Self(PhantomData)
                    }
                }

                trait Fallback<#subject_generic>: Sized {
                    #[allow(dead_code)]
                    fn implements(&self) -> bool { false }
                    #[allow(dead_code)]
                    fn get #carried (self, _: #probe_shape #(, _: #types)*) -> #handed_back
                    #carried_where
                    { #unreached }
                }
                impl<#subject_generic> Fallback<#subject_generic> for Probe<#subject_generic> {}

                impl<#subject_generic: #subject_bounds> Probe<#subject_generic> {
                    #[allow(dead_code)]
                    fn implements(&self) -> bool { true }
                    #[allow(dead_code)]
                    fn get #carried (
                        self, subject: #probe_shape #(, #names: #types)*
                    ) -> #handed_back
                    #carried_where
                    {
                        #name #forwarded (subject #(, #names)*)
                    }
                }

                let probe = Probe::new(&$value);
                if probe.implements() {
                    Option::Some(
                        move |subject #(, #names)*| probe.get #turbofish (subject #(, #names)*)
                    )
                } else {
                    Option::None
                }
            }
        };
    };

    // Named for what it does rather than for the function, since it hands back a
    // function to call rather than calling one. Its own name also lets it be
    // re-exported as it stands: `use #name;` would name every namespace at once and
    // collide with the function itself (`E0255`).

    // Written here rather than on the alias: an editor resolves the invocation to
    // the `macro_rules!` and reads the documentation from there, and would find
    // nothing on a bare re-export. It avoids the words this crate uses for its own
    // parts, since whoever calls this may never have seen them.
    // The bounds themselves, so the summary says what has to hold rather than
    // pointing at the signature for it.
    let bounds = render(&subject_bounds);
    let summary = format!(
        "Tries to instantiate [`{name}`] for the type of an expression, if that type satisfies \
         the bounds on its first parameter (`{bounds}`)."
    );
    let returns = match asyncness {
        Some(_) => format!(
            "Returns `Some` of an `Fn` for that instantiation, or `None` if the bounds do not hold. \
             That `Fn` yields [`{name}`]'s future, so nothing runs until it is called and awaited."
        ),
        None => {
            "Returns `Some` of an `Fn` for that instantiation, or `None` if the bounds do not hold."
                .to_string()
        }
    };

    let arguments: Vec<String> = std::iter::once(subject_name.clone())
        .chain(names.iter().map(|name| name.to_string()))
        .collect();
    let arguments = arguments.join(", ");
    let awaited = if asyncness.is_some() { ".await" } else { "" };
    // The types are shown only where they are not optional, so the example stays the
    // shortest call that compiles.
    let given = match required.is_empty() {
        true => String::new(),
        false => format!("; {placeholders}"),
    };
    // `ignore`, which rustdoc renders as Rust and does not compile. Nothing here
    // could be compiled: the arguments stand for what the caller passes, and a
    // doctest of the crate this expands in cannot name the function anyway.
    let usage = format!(
        "```ignore\n\
         match {macro_name}!({subject_name}{given}) {{\n    \
             Some(f) => f({arguments}){awaited},\n    \
             None => {{ /* `{bounds}` does not hold */ }}\n\
         }}\n\
         ```"
    );
    let called = format!(
        "`f` takes the arguments [`{name}`] declares, and is an [`Fn`][::core::ops::Fn], \
        so it may be called more than once."
    );
    // The trap worth spelling out: nothing is moved, so there is no reason to add a
    // `&` out of caution, and one added anyway is not an error but a different type
    // to test -- `&u8` satisfies `Display` as readily as `u8` does, and instantiates
    // for the reference instead.
    let ownership = format!(
        "Only the type is read here, so nothing is moved: write the expression as you would write \
         the argument. Passing a reference (`&{subject_name}` instead of `{subject_name}`) tests a \
         different type, which might not satisfy the bounds."
    );
    let generic = "The bounds are tested against what the expression's type is known to be here, \
                   which inside a generic function is whatever that function declares: a parameter \
                   it does not bound never satisfies them.";
    let scope = format!(
        "[`{name}`] and the traits its bounds name have to be in scope where this is called, \
         because a `macro_rules!` body resolves its paths at the call site."
    );
    // Said only where the `;` accepts something: on a function whose only type
    // parameter is the first one's, there is nothing to fix and nothing to explain.
    let generics = (!carried_args.is_empty()).then(|| {
        let slots = carried_args
            .iter()
            .map(|ident| ident.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let text = format!(
            "The parameters [`{name}`] declares besides the first can be given after a '`;`' in \
             their declaration order (`{slots}`), or left to inference as \
             they would be at any other call."
        );
        quote! {
            #[doc = ""]
            #[doc = #text]
        }
    });
    let docs = quote! {
        #[doc = #summary]
        #[doc = ""]
        #[doc = #returns]
        #[doc = ""]
        #[doc = #usage]
        #[doc = ""]
        #[doc = #called]
        #[doc = ""]
        #[doc = #ownership]
        #[doc = ""]
        #[doc = #generic]
        #generics
        #[doc = ""]
        #[doc = #scope]
    };

    // One shape, whatever the visibility: the macro stays in the crate, so there is
    // no `#[macro_export]` and nothing lands in the crate root. The re-export beside
    // it is what makes it nameable by path at all, a `macro_rules!` being otherwise
    // visible only to what follows it in the same file.
    let generated = quote! {
        #docs
        macro_rules! #macro_name {
            #rule
        }

        // Pulls those same docs onto the re-export, which is what a reader reaches,
        // instead of a bare line.
        #[doc(inline)]
        #[allow(unused_imports)]
        #macro_vis use #macro_name;
    };

    quote! {
        #item

        #generated
    }
}
