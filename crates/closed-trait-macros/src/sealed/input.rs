use proc_macro2::{Span, TokenStream};
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
        undeclared_lifetimes(&types, &item)?;
        unpinned_entries(&types, &item)?;
        mismatched_instantiations(&types, &item)?;

        Ok(Input { item, types })
    }
}

/// An instantiation annotation has to name the trait being sealed.
///
/// Only its arguments are ever read, so another name would sit there looking
/// meaningful while doing nothing — and a typo would never be noticed.
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
/// trait's type and const parameters supplied for the entry: either the type
/// names them itself, as `Boxed<T>` does under `trait Store<T>`, or the entry
/// annotates them, as in `Plain: Store<i32>`.
///
/// Leaving it to inference nearly works — it finds the answer when there is a
/// single impl, and reports a missing one — but a type implementing the trait
/// at several instantiations gives `E0283: type annotations needed`, spanned on
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

    for entry in types {
        if entry.instantiation.is_some() {
            continue;
        }

        let bound: Vec<String> = entry
            .binder
            .iter()
            .flat_map(|binder| binder.params.iter())
            .map(name_of)
            .collect();
        let missing: Vec<&str> = needed
            .iter()
            .filter(|name| !bound.contains(name) && !mentions(&entry.ty, name))
            .map(String::as_str)
            .collect();

        if !missing.is_empty() {
            let ty = render(&entry.ty);
            let trait_ = &item.ident;
            let named = missing.join("`, `");
            let example = needed.iter().map(|_| "..").collect::<Vec<_>>().join(", ");
            return Err(Error::new_spanned(
                &entry.ty,
                format!(
                    "`{ty}` does not say which `{trait_}` it implements: it names neither \
                     `{named}` nor an instantiation, so nothing can check that it implements \
                     `{trait_}` at all.\nWrite `{ty}: {trait_}<{example}>` with the arguments \
                     it implements, or name the trait's parameters in the type itself"
                ),
            ));
        }
    }

    Ok(())
}

/// A lifetime an entry names has to come from somewhere.
///
/// Left to itself the macro would declare it, which quietly turns the entry
/// into a claim about *every* lifetime — and makes renaming the trait's own
/// parameter change what is sealed, since a name that matches the trait's
/// carries the trait's bounds while one that does not carries none. `for<..>`
/// says which was meant, exactly as it does for a type. `'_` and `'static` are
/// not names to be confused with anything, so they pass.
fn undeclared_lifetimes(types: &[SealedType], item: &ItemTrait) -> Result<()> {
    let declared: Vec<String> = item.generics.params.iter().map(name_of).collect();

    for entry in types {
        let bound: Vec<String> = entry
            .binder
            .iter()
            .flat_map(|binder| binder.params.iter())
            .map(name_of)
            .collect();

        for lifetime in lifetimes(&entry.ty) {
            let name = lifetime.to_string();
            if bound.contains(&name) || declared.contains(&name) {
                continue;
            }

            return Err(Error::new(
                lifetime.span(),
                format!(
                    "`'{name}` is neither declared by `{trait_}` nor bound by a `for<..>`, so \
                     this entry would quietly mean every `'{name}`.\nWrite \
                     `for<'{name}> {ty}` if that is what you meant, or name one of \
                     `{trait_}`'s own lifetimes",
                    trait_ = item.ident,
                    ty = render(&entry.ty),
                ),
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

        if types.is_empty() {
            return Err(Error::new(
                Span::call_site(),
                "`#[sealed(..)]` needs the types allowed to implement the trait, \
                 as in `#[sealed(Square, crate::Circle)]`",
            ));
        }

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
    fn an_empty_list_is_refused() {
        assert!(refused(quote!(), plain()).contains("needs the types allowed"));
    }

    #[test]
    fn options_are_refused() {
        assert!(refused(quote!(name = Shapes), plain()).contains("takes no options"));
    }

    #[test]
    fn an_undeclared_lifetime_is_refused() {
        let message = refused(quote!(Slice<'b>), plain());
        assert!(message.contains("neither declared by `Shape`"));
        assert!(message.contains("for<'b> Slice<'b>"), "names the fix");
    }

    #[test]
    fn a_lifetime_the_trait_declares_is_accepted() {
        let item: ItemTrait = parse_quote!(
            pub trait Text<'a> {}
        );
        assert_eq!(accepted(quote!(Slice<'a>), item).len(), 1);
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
    fn naming_the_parameter_or_annotating_it_both_satisfy_the_check() {
        let item: ItemTrait = parse_quote!(
            pub trait Store<T> {}
        );
        assert_eq!(accepted(quote!(Boxed<T>), item.clone()).len(), 1);
        assert_eq!(accepted(quote!(Plain: Store<i32>), item).len(), 1);
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
