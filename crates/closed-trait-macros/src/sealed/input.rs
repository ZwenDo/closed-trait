use proc_macro2::TokenStream;
use syn::parse::{ParseStream, Parser};
use syn::{Error, GenericParam, Generics, Ident, ItemTrait, Path, Result, Token, Type};

use crate::util::{lifetimes, mentions, name_of, render};

/// A validated `#[sealed(..)]` invocation.
pub(crate) struct Input {
    /// The trait being sealed, still exactly as written.
    pub(crate) item: ItemTrait,
    /// The types allowed to implement it.
    pub(crate) types: Vec<SealedType>,
}

impl Input {
    pub(crate) fn parse(args: TokenStream, item: ItemTrait) -> Result<Self> {
        let Args { types } = Args::parse(args)?;
        undeclared_lifetimes(&types)?;
        unused_binder_parameters(&types)?;
        unpinned_entries(&types, &item)?;
        mismatched_instantiations(&types, &item)?;

        Ok(Input { item, types })
    }
}

/// An instantiation annotation has to name the trait being sealed.
///
/// Only its arguments are ever read, so another name would sit there looking
/// meaningful while doing nothing, and a typo would never be noticed.
fn mismatched_instantiations(types: &[SealedType], item: &ItemTrait) -> Result<()> {
    for entry in types {
        let Some(path) = &entry.instantiation else {
            continue;
        };
        let Some(segment) = path.segments.last() else {
            continue;
        };
        if segment.ident == item.ident {
            continue;
        }

        let ty = render(&entry.ty);
        let sealed = &item.ident;
        return Err(Error::new_spanned(
            path,
            format!(
                "`{}` is not the trait being sealed. The annotation says how `{ty}` implements \
                 `{sealed}`, so write `{ty}: {sealed}{}`",
                segment.ident,
                render(&segment.arguments),
            ),
        ));
    }

    Ok(())
}

/// An entry has to say which instantiation of a generic trait it implements.
///
/// The list is checked in both directions, and the restrictive half needs the
/// trait's type and const parameters supplied for the entry, which only its
/// instantiation does: `Plain: Store<i32>` pins them, `for<T> Boxed<T>: Store<T>`
/// takes them from the binder.
///
/// Leaving it to inference nearly works: it finds the answer when there is a
/// single impl, and reports a missing one. But a type implementing the trait
/// at several instantiations gives "type annotations needed", spanned on
/// a generated function the caller never wrote. Refusing here costs one
/// annotation and says what to write.
fn unpinned_entries(types: &[SealedType], item: &ItemTrait) -> Result<()> {
    // Lifetimes are exempt: a type cannot implement the same trait at two
    // different lifetimes, two such impls overlapping and being rejected by
    // coherence, so there is never more than one candidate to infer.
    let needed: Vec<String> = item
        .generics
        .params
        .iter()
        .filter(|param| !matches!(param, GenericParam::Lifetime(_)))
        .map(name_of)
        .collect();
    if needed.is_empty() {
        return Ok(());
    }

    // Only the instantiation says it now: a bare name in the entry is whatever is
    // in scope where it was written, never the trait's parameter of that name.
    for entry in types {
        if entry.instantiation.is_some() {
            continue;
        }

        return Err(needs_instantiation(&entry.ty, item));
    }

    Ok(())
}

/// The refusal for an entry that says nothing about which instantiation it
/// implements.
///
/// Shared with `#[enumerate]`, which refuses the same entry for its own reason:
/// an identical message at an identical span is one diagnostic rather than two,
/// and the fix is the same either way.
pub(crate) fn needs_instantiation(entry: &Type, item: &ItemTrait) -> Error {
    let arguments = item
        .generics
        .params
        .iter()
        .filter(|param| !matches!(param, GenericParam::Lifetime(_)))
        .map(|_| "..")
        .collect::<Vec<_>>()
        .join(", ");
    let ty = render(entry);
    let trait_ = &item.ident;
    Error::new_spanned(
        entry,
        format!(
            "`{ty}` does not say which `{trait_}` it implements, so nothing can check that it \
             implements `{trait_}` at all.\nWrite `{ty}: {trait_}<{arguments}>` with the \
             arguments it implements"
        ),
    )
}

/// A lifetime an entry names has to be bound by its own `for<..>`.
///
/// Left to itself the macro would declare it, which quietly turns the entry into
/// a claim about *every* lifetime. The trait's own are not in scope here, no
/// more than its type parameters are: a bare name is whatever is in scope where
/// the attribute was written. `'_` and `'static` are not names to be confused
/// with anything, so they pass.
fn undeclared_lifetimes(types: &[SealedType]) -> Result<()> {
    for entry in types {
        let bound: Vec<String> = entry
            .binder
            .iter()
            .flat_map(|binder| binder.params.iter())
            .map(name_of)
            .collect();

        for lifetime in lifetimes(&entry.ty) {
            let name = lifetime.to_string();
            if bound.contains(&name) {
                continue;
            }

            return Err(Error::new(
                lifetime.span(),
                format!(
                    "`'{name}` is not bound by a `for<..>`, so this entry would quietly mean \
                     every `'{name}`.\nWrite `for<'{name}> {ty}` if that is what you meant",
                    ty = render(&entry.ty),
                ),
            ));
        }
    }

    Ok(())
}

/// A parameter a `for<..>` declares has to be used by the entry.
///
/// The generated items declare it either way, so an unused one is at best
/// nothing and at worst a refusal from rustc: an unconstrained type or const
/// parameter on an impl is an error, spanned on the attribute rather than on
/// the binder, while an unconstrained lifetime is quietly allowed and means
/// nothing at all.
///
/// Used means named by the type or by the instantiation, `for<T> Plain:
/// Store<T>` being the shape that uses one only in the latter. A parameter
/// named solely in another's bounds does not count: it constrains nothing, and
/// the impl rustc writes from it is the unconstrained case again.
fn unused_binder_parameters(types: &[SealedType]) -> Result<()> {
    for entry in types {
        let Some(binder) = &entry.binder else {
            continue;
        };

        for param in &binder.params {
            let name = name_of(param);
            let used = match param {
                GenericParam::Lifetime(_) => {
                    let named = |found: Vec<Ident>| found.iter().any(|found| *found == name);
                    named(lifetimes(&entry.ty))
                        || entry
                            .instantiation
                            .as_ref()
                            .is_some_and(|path| named(lifetimes(path)))
                }
                _ => {
                    mentions(&entry.ty, &name)
                        || entry
                            .instantiation
                            .as_ref()
                            .is_some_and(|p| mentions(p, &name))
                }
            };
            if used {
                continue;
            }

            let written = match param {
                GenericParam::Lifetime(_) => format!("'{name}"),
                _ => name.clone(),
            };
            // The instantiation is the other place it could have been used, and
            // naming it is only possible where there is one.
            let places = match entry.instantiation.as_ref().map(render) {
                Some(instantiation) => format!("`{}` or `{instantiation}`", render(&entry.ty)),
                None => format!("`{}`", render(&entry.ty)),
            };
            return Err(Error::new_spanned(
                param,
                format!("`{written}` is declared by the `for<..>` but not constrained by {places}"),
            ));
        }
    }

    Ok(())
}

/// The contents of a `#[sealed(..)]` attribute, split out so that other macros
/// can read the type list off a trait that is still carrying the attribute.
pub(crate) struct Args {
    pub(crate) types: Vec<SealedType>,
}

/// One entry of the list, optionally quantified: `for<T> Boxed<T>` seals every
/// `Boxed`, which a trait that declares no parameters of its own could not
/// otherwise express.
pub(crate) struct SealedType {
    pub(crate) binder: Option<Generics>,
    pub(crate) ty: Type,
    /// The variant name to use instead of the type's last path segment, as in
    /// `a::Foo as AFoo`. Only `#[enumerate]` looks at it.
    pub(crate) alias: Option<Ident>,
    /// How this type instantiates the trait, as in `Plain: Store<i32>`.
    ///
    /// Only needed when the type does not name the trait's parameters itself:
    /// without it there is no way to know which `Store` a `Plain` implements.
    pub(crate) instantiation: Option<Path>,
}

impl Args {
    pub(crate) fn parse(args: TokenStream) -> Result<Self> {
        let mut types = Vec::new();
        let parser = |stream: ParseStream| parse_args(stream, &mut types);
        parser.parse2(args)?;

        // An empty list is a trait nothing may implement, which is a seal like any
        // other and the state a list is in before it has been filled. `#[enumerate]`
        // refuses it separately, having no variants to make an enum from.
        Ok(Args { types })
    }
}

/// A comma separated list of types.
fn parse_args(stream: ParseStream, types: &mut Vec<SealedType>) -> Result<()> {
    while !stream.is_empty() {
        // No options are accepted, but `key = value` is worth recognising so
        // that a mistaken one says so rather than failing as a malformed type.
        if stream.peek(Ident) && stream.peek2(Token![=]) {
            let key: Ident = stream.parse()?;
            return Err(Error::new_spanned(
                &key,
                format!("`#[sealed(..)]` takes no options, but found `{key} = ..`"),
            ));
        } else {
            // `for` is a keyword, so it never collides with the option branch
            // above, which only fires on an identifier followed by `=`.
            let binder = if stream.peek(Token![for]) {
                let keyword = stream.parse::<Token![for]>()?;
                // A binder holds declarations, not arguments, so a const
                // parameter needs its type just as it would anywhere else.
                // syn's own message for that is a bare `expected ':'`.
                Some(stream.parse::<Generics>().map_err(|error| {
                    Error::new(
                        error.span(),
                        format!(
                            "{error}\n`{}<..>` declares parameters, as in \
                             `for<'a, T: Clone, const N: usize>`",
                            quote::ToTokens::to_token_stream(&keyword),
                        ),
                    )
                })?)
            } else {
                None
            };
            let ty = stream.parse()?;
            let alias = if stream.peek(Token![as]) {
                stream.parse::<Token![as]>()?;
                Some(stream.parse()?)
            } else {
                None
            };
            let instantiation = if stream.peek(Token![:]) {
                stream.parse::<Token![:]>()?;
                Some(stream.parse()?)
            } else {
                None
            };
            types.push(SealedType {
                binder,
                ty,
                alias,
                instantiation,
            });
        }

        if stream.is_empty() {
            break;
        }
        stream.parse::<Token![,]>()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;
    use syn::parse_quote;

    /// `Input` has no `Debug`, syn's own impls being behind a feature, so the
    /// outcome comes out by hand rather than through `unwrap`/`expect_err`.
    fn refused(attr: TokenStream, item: ItemTrait) -> String {
        match Input::parse(attr, item) {
            Err(error) => error.to_string(),
            Ok(_) => panic!("expected the attribute to be refused"),
        }
    }

    fn accepted(attr: TokenStream, item: ItemTrait) -> Vec<SealedType> {
        match Input::parse(attr, item) {
            Ok(input) => input.types,
            Err(error) => panic!("expected the attribute to be accepted: {error}"),
        }
    }

    fn plain() -> ItemTrait {
        parse_quote!(
            pub trait Shape {}
        )
    }

    #[test]
    fn an_entry_may_be_just_a_type() {
        let types = accepted(quote!(Square, a::Circle), plain());
        assert_eq!(types.len(), 2);
        assert!(types.iter().all(|entry| entry.binder.is_none()));
        assert!(types.iter().all(|entry| entry.alias.is_none()));
        assert!(types.iter().all(|entry| entry.instantiation.is_none()));
        assert_eq!(render(&types[1].ty), "a::Circle");
    }

    #[test]
    fn an_entry_may_use_every_part_at_once() {
        let item: ItemTrait = parse_quote!(
            pub trait Store<T> {}
        );
        let types = accepted(
            quote!(for<'x, U: Clone> Shown<'x, U> as Displayed: Store<i32>),
            item,
        );

        let entry = &types[0];
        let binder = entry.binder.as_ref().expect("a binder");
        assert_eq!(binder.params.len(), 2, "the lifetime and the type");
        assert_eq!(render(&entry.ty), "Shown<'x, U>");
        assert_eq!(
            entry.alias.as_ref().expect("an alias").to_string(),
            "Displayed"
        );
        assert_eq!(
            render(entry.instantiation.as_ref().expect("an instantiation")),
            "Store<i32>"
        );
    }

    #[test]
    fn an_empty_list_seals_the_trait_against_everything() {
        let args = Args::parse(quote!()).expect("an empty list parses");
        assert!(args.types.is_empty());
    }

    #[test]
    fn options_are_refused() {
        assert!(refused(quote!(name = Shapes), plain()).contains("takes no options"));
    }

    #[test]
    fn an_undeclared_lifetime_is_refused() {
        let message = refused(quote!(Slice<'b>), plain());
        assert!(message.contains("not bound by a `for<..>`"), "{message}");
        assert!(message.contains("for<'b> Slice<'b>"), "names the fix");
    }

    /// The trait's own lifetimes are not in scope in an entry either: a bare
    /// name is whatever the attribute was written beside.
    #[test]
    fn a_lifetime_the_trait_declares_is_refused_all_the_same() {
        let item: ItemTrait = parse_quote!(
            pub trait Text<'a> {}
        );
        let message = refused(quote!(Slice<'a>), item);
        assert!(message.contains("not bound by a `for<..>`"), "{message}");
    }

    /// A binder declares parameters for the entry to use. One it does not use
    /// leaves the generated impls declaring something they never constrain,
    /// which rustc refuses for a type or a const and quietly allows -- meaning
    /// nothing -- for a lifetime.
    #[test]
    fn an_unused_binder_parameter_is_refused() {
        let message = refused(quote!(for<'a> Bar), plain());
        assert!(
            message.contains("`'a` is declared by the `for<..>`"),
            "{message}"
        );
        assert!(message.contains("not constrained by `Bar`"), "{message}");

        let message = refused(quote!(for<T> Held<i32>), plain());
        assert!(
            message.contains("`T` is declared by the `for<..>`"),
            "{message}"
        );

        let item: ItemTrait = parse_quote!(
            pub trait Width<const M: usize> {}
        );
        let message = refused(quote!(for<const N: usize> Row: Width<3>), item);
        assert!(
            message.contains("`N` is declared by the `for<..>`"),
            "{message}"
        );
    }

    /// The instantiation counts as use, which is the whole of what
    /// `for<T> Plain: Store<T>` says.
    #[test]
    fn a_parameter_used_only_in_the_instantiation_is_accepted() {
        let item: ItemTrait = parse_quote!(
            pub trait Store<T> {}
        );
        assert_eq!(
            accepted(quote!(for<T> Plain: Store<T>), item.clone()).len(),
            1
        );
        assert_eq!(
            accepted(quote!(for<'a> Plain: Store<&'a str>), item).len(),
            1
        );
    }

    /// A name used only in another parameter's bounds constrains nothing, so
    /// the impl written from it is the unconstrained case again.
    #[test]
    fn a_parameter_used_only_in_a_bound_is_refused() {
        let message = refused(quote!(for<T, U: Into<T>> Held<U>), plain());
        assert!(
            message.contains("`T` is declared by the `for<..>`"),
            "{message}"
        );
    }

    #[test]
    fn a_bound_lifetime_is_accepted() {
        assert_eq!(accepted(quote!(for<'b> Slice<'b>), plain()).len(), 1);
    }

    #[test]
    fn an_instantiation_must_name_the_sealed_trait() {
        let item: ItemTrait = parse_quote!(
            pub trait Store<T> {}
        );
        let message = refused(quote!(Plain: Az<i32>), item);
        assert!(message.contains("`Az` is not the trait being sealed"));
        assert!(message.contains("Plain: Store<i32>"), "names the fix");
    }

    #[test]
    fn a_qualified_instantiation_is_accepted() {
        let item: ItemTrait = parse_quote!(
            pub trait Store<T> {}
        );
        assert_eq!(accepted(quote!(Plain: crate::Store<i32>), item).len(), 1);
    }

    #[test]
    fn an_entry_that_pins_nothing_is_refused() {
        let item: ItemTrait = parse_quote!(
            pub trait Store<T> {}
        );
        let message = refused(quote!(Plain), item);
        assert!(message.contains("does not say which `Store` it implements"));
    }

    #[test]
    fn only_an_instantiation_says_which_trait_an_entry_implements() {
        let item: ItemTrait = parse_quote!(
            pub trait Store<T> {}
        );
        // Naming the trait's parameter in the type says nothing: `T` there is
        // whatever `T` is in scope where the attribute was written.
        let message = refused(quote!(Boxed<T>), item.clone());
        assert!(message.contains("does not say which `Store`"), "{message}");

        assert_eq!(accepted(quote!(Plain: Store<i32>), item.clone()).len(), 1);
        assert_eq!(accepted(quote!(for<U> Boxed<U>: Store<U>), item).len(), 1);
    }

    /// A trait parameterised only by lifetimes asks nothing of its entries:
    /// coherence forbids two impls differing only in a lifetime, so there is
    /// never more than one candidate to infer.
    #[test]
    fn a_lifetime_only_trait_pins_nothing() {
        let item: ItemTrait = parse_quote!(
            pub trait Held<'a> {}
        );
        assert_eq!(accepted(quote!(Plain), item).len(), 1);
    }
}
