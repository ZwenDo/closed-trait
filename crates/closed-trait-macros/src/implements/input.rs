use crate::util::{fresh, name_of};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    FnArg, GenericParam, Ident, ItemFn, LitStr, Token, Type, TypeParamBound, Visibility,
    WherePredicate, parse_quote,
};

const VIS: &str = "vis";
const NAME: &str = "name";

/// The signature, read for what the generated macro has to know: which bounds it
/// tests, on what shape, and how far the macro may be seen.
pub(crate) struct Input {
    /// The function, still exactly as written. It is emitted unchanged.
    pub(crate) item: ItemFn,
    /// The macro's visibility, which is the function's capped at the crate.
    pub(crate) macro_vis: Visibility,
    /// What the macro is called: `try_` before the function's name unless the
    /// attribute said otherwise.
    pub(crate) macro_name: Ident,
    /// The type parameter under test: the one the first parameter's annotation
    /// names, or one invented for an `impl Bound`.
    pub(crate) subject: Ident,
    /// The first parameter's type, with `subject` where `impl Bound` was written.
    pub(crate) shape: TokenStream,
    /// The bounds under test, inline ones and `where` predicates together.
    pub(crate) bounds: TokenStream,
    /// The `where` predicates that say nothing about the subject, which belong to
    /// the method the call supplies its arguments to rather than to the probe.
    pub(crate) carried_where: Option<TokenStream>,
}

impl Input {
    pub(crate) fn parse(args: TokenStream, item: ItemFn) -> syn::Result<Self> {
        let Options { vis: asked, name } = Options::parse.parse2(args)?;
        check(&item)?;

        let signature = &item.sig;

        // The macro's visibility, capped at the crate whatever the function's is. A
        // `macro_rules!` leaves its crate only through `#[macro_export]`, and that
        // plants its name in the crate *root*, one per function and regardless of the
        // module it was written in, which two functions sharing a name could not both
        // have.
        let macro_vis: Visibility = match asked {
            Some(Visibility::Public(token)) => {
                return Err(syn::Error::new_spanned(
                    token,
                    format!(
                        "`{VIS} = \"pub\"` is not currently supported: the macro's visibility \
                         ranges from `\"pub(self)\"` to `\"pub(crate)\"`"
                    ),
                ));
            }
            Some(asked) => asked,
            None => match &item.vis {
                Visibility::Public(_) => parse_quote!(pub(crate)),
                narrower => narrower.clone(),
            },
        };

        // The first parameter is the one tested; `check` has already insisted on it.
        let annotation = match signature.inputs.first() {
            Some(FnArg::Typed(pat)) => &pat.ty,
            _ => unreachable!("`check` refused every other shape"),
        };

        // Which parameter is its type: the one the annotation names. An `impl Bound`
        // annotation names none, being an anonymous parameter, so one is invented and
        // the bounds come from the annotation itself.
        let named = signature
            .generics
            .params
            .iter()
            .find_map(|param| match param {
                GenericParam::Type(ty) if mentions(quote!(#annotation), &ty.ident) => {
                    Some(ty.ident.clone())
                }
                _ => None,
            });
        let (subject, shape, anonymous_bounds) = match named {
            Some(ident) => (ident, quote!(#annotation), None),
            None => {
                // `T` unless the function declares one already: the probe declares
                // this parameter and `get` declares the rest, so a name used twice
                // would quietly swallow the function's own.
                let invented = fresh(signature.generics.params.iter().map(name_of), "T");
                let (rewritten, bounds) =
                    shape_of((**annotation).clone(), &invented).map_err(|_| {
                        syn::Error::new(
                            annotation.span(),
                            "the first parameter's type has to be a type parameter this function \
                             declares, or an `impl Bound` standing for one, so that there are \
                             bounds to test: `fn f<T: Bound>(v: &T)` and `fn f(v: &impl Bound)` \
                             both work",
                        )
                    })?;
                (invented, rewritten, Some(bounds))
            }
        };

        // A `where` predicate about the subject belongs to the probe; the rest belong
        // to the method the call supplies its arguments to, which is where their
        // parameters are declared.
        let (tested, carried): (Vec<&WherePredicate>, Vec<&WherePredicate>) = signature
            .generics
            .where_clause
            .iter()
            .flat_map(|clause| clause.predicates.iter())
            .partition(|predicate| match predicate {
                WherePredicate::Type(ty) => mentions(quote!(#ty), &subject),
                _ => false,
            });
        let carried_where = (!carried.is_empty()).then(|| quote!(where #(#carried),*));

        let inline = signature
            .generics
            .params
            .iter()
            .find_map(|param| match param {
                GenericParam::Type(ty) if ty.ident == subject => Some(&ty.bounds),
                _ => None,
            });
        let extra: Vec<TokenStream> = tested
            .iter()
            .filter_map(|predicate| match predicate {
                WherePredicate::Type(ty) => {
                    let bounds = &ty.bounds;
                    Some(quote!(#bounds))
                }
                _ => None,
            })
            .collect();
        let bounds = match (&anonymous_bounds, inline, extra.is_empty()) {
            // `impl Bound` carries its own, and declares no parameter for a `where`
            // predicate to name.
            (Some(bounds), _, _) => quote!(#bounds),
            (None, Some(bounds), true) => quote!(#bounds),
            (None, Some(bounds), false) if !bounds.is_empty() => quote!(#bounds + #(#extra)+*),
            (None, _, false) => quote!(#(#extra)+*),
            _ => quote!(),
        };

        // Named for what it does rather than for the function, since it hands back
        // a function to call rather than calling one.
        let macro_name = name.unwrap_or_else(|| format_ident!("try_{}", signature.ident));

        Ok(Self {
            item,
            macro_vis,
            macro_name,
            subject,
            shape,
            bounds,
            carried_where,
        })
    }
}

/// Whether `tokens` name `ident` anywhere, at any nesting.
/// Whether `tokens` name `ident` anywhere, at any nesting.
pub(crate) fn mentions(tokens: TokenStream, ident: &Ident) -> bool {
    tokens.into_iter().any(|tt| match tt {
        proc_macro2::TokenTree::Ident(other) => &other == ident,
        proc_macro2::TokenTree::Group(group) => mentions(group.stream(), ident),
        _ => false,
    })
}

/// A name for the first parameter's type where the annotation gives none, taken
/// from the same alphabet a signature would use.
///
/// `T` unless the function declares one already, since the two stand side by
/// side: the probe declares this one, `get` declares the rest, and a name used
/// twice would quietly swallow the function's own. Nothing else in the expansion
/// needs guarding, the function's body being elsewhere.
fn fresh(signature: &syn::Signature) -> Ident {
    let declared: Vec<&Ident> = signature
        .generics
        .params
        .iter()
        .filter_map(|param| match param {
            GenericParam::Lifetime(_) => None,
            GenericParam::Type(ty) => Some(&ty.ident),
            GenericParam::Const(constant) => Some(&constant.ident),
        })
        .collect();
    let span = proc_macro2::Span::call_site();
    let mut candidate = Ident::new("T", span);
    let mut suffix = 1;
    while declared.contains(&&candidate) {
        suffix += 1;
        candidate = Ident::new(&format!("T{suffix}"), span);
    }
    candidate
}

/// The attribute's options, `vis = ..` and `name = ..`, in any order and each
/// written at most once.
#[derive(Default)]
struct Options {
    /// The visibility to give the macro, where the one it takes from the function
    /// is not the one wanted. `pub` is the one value that cannot be asked for,
    /// and `Input::parse` says why.
    vis: Option<Visibility>,
    /// What to call the macro, where `try_` before the function's name is not
    /// wanted -- because something else already has that name, or because it
    /// reads badly on this particular function.
    name: Option<Ident>,
}

impl Parse for Options {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut options = Options::default();
        while !input.is_empty() {
            // A visibility on its own is the shape to expect someone to reach for,
            // and `pub` being a keyword it would otherwise be refused as "not an
            // identifier".
            if input.peek(Token![pub]) {
                return Err(input.error(format!(
                    "expected `{VIS} = \"..\"`, as in `{VIS} = \"pub(crate)\"`"
                )));
            }
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            // Both values are written as strings, so what is inside them is read by
            // the same parser that reads the real thing, and a keyword like `pub`
            // needs no special case at this level.
            let literal: LitStr = input.parse()?;
            match () {
                _ if key == VIS => {
                    // Anything but `pub` parses as `Inherited` without consuming a
                    // token, so what follows is left over and syn reports a stray
                    // token rather than the wrong word. A malformed `pub(..)` is
                    // still syn's to explain.
                    let visibility: Visibility =
                        literal.parse().map_err(|error| {
                            match literal.value().trim_start().starts_with("pub") {
                                true => error,
                                false => syn::Error::new(
                                    literal.span(),
                                    "expected a visibility, such as `\"pub(crate)\"`, \
                                 `\"pub(super)\"` or `\"pub(self)\"`",
                                ),
                            }
                        })?;
                    // An empty string parses as `Inherited` and consumes nothing, so
                    // it reaches here rather than erroring above.
                    if matches!(visibility, Visibility::Inherited) {
                        return Err(syn::Error::new(
                            literal.span(),
                            "expected a visibility, such as `\"pub(crate)\"`, `\"pub(super)\"` or \
                             `\"pub(self)\"`",
                        ));
                    }
                    if options.vis.replace(visibility).is_some() {
                        return Err(syn::Error::new(
                            key.span(),
                            format!("`{VIS}` is written twice"),
                        ));
                    }
                }
                _ if key == NAME => {
                    let name: Ident = literal.parse()?;
                    if options.name.replace(name).is_some() {
                        return Err(syn::Error::new(
                            key.span(),
                            format!("`{NAME}` is written twice"),
                        ));
                    }
                }
                _ => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown option `{key}`, expected `{VIS}` or `{NAME}`"),
                    ));
                }
            }
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }
        Ok(options)
    }
}

/// The signature must be one a `macro_rules!` can stand beside, and one the probe
/// can take apart. Everything refused here is refused for a reason rustc would
/// otherwise report from inside the expansion.
fn check(item: &ItemFn) -> syn::Result<()> {
    let signature = &item.sig;
    // A `const fn` is accepted and its constness left alone: the function is emitted
    // as written, so a direct call is still const, and only the macro's path is not.
    // What that path hands back is an `Fn` called at run time, so nothing here needs
    // to be const -- which is as well, since the fallback is a trait method and those
    // cannot be (`E0379`).
    if let syn::Safety::Unsafe(unsafety) = signature.safety {
        return Err(syn::Error::new(
            unsafety.span(),
            "an `unsafe fn` cannot be used here: what this hands back is safe to call, which would \
             hide the unsafety rather than carry it",
        ));
    }
    if let Some(abi) = &signature.abi {
        return Err(syn::Error::new(
            abi.span(),
            "an explicit ABI cannot be used here: the body is called as an ordinary function, so \
             the ABI would say something that is not true of the call",
        ));
    }
    // Defensive, and not reachable from stable: a C-variadic can only be *defined*
    // under an unstable feature, and needs the `unsafe` and the ABI that are refused
    // just above. Left in so that a nightly signature syn can parse is refused here
    // rather than somewhere inside the expansion.
    if let Some(variadic) = &signature.variadic {
        return Err(syn::Error::new(
            variadic.span(),
            "a C-variadic cannot be used here: what this hands back is an ordinary `Fn`, which has \
             no way to carry the extra arguments",
        ));
    }
    match signature.inputs.first() {
        None => Err(syn::Error::new(
            signature.paren_token.span.join(),
            "the first parameter is the one whose bounds are tested, so there has to be one",
        )),
        Some(FnArg::Receiver(receiver)) => Err(syn::Error::new(
            receiver.span(),
            "this has to be a free function, not a method: the macro it generates sits beside it, \
             and a `macro_rules!` cannot be defined in an `impl` or a `trait`",
        )),
        Some(FnArg::Typed(_)) => Ok(()),
    }
}

/// Rewrites the annotation into the shape the first parameter is taken at,
/// replacing `impl Bound` with `name` and leaving every lifetime elided. Any
/// depth works, so `&mut &&impl Foo` is as ordinary as `&impl Foo`.
///
/// The name is the caller's, since it stands beside whatever the function
/// declares and has to avoid it.
pub(crate) fn shape_of(
    annotation: Type,
    name: &Ident,
) -> syn::Result<(TokenStream, Punctuated<TypeParamBound, Token![+]>)> {
    fn walk(
        ty: Type,
        name: &Ident,
    ) -> Option<(TokenStream, Punctuated<TypeParamBound, Token![+]>)> {
        match ty {
            Type::ImplTrait(it) => Some((quote! { #name }, it.bounds)),
            Type::Paren(paren) => walk(*paren.elem, name),
            Type::Group(group) => walk(*group.elem, name),
            Type::Reference(reference) => {
                let mutability = reference.mutability;
                let (inner, bounds) = walk(*reference.elem, name)?;
                Some((quote! { &#mutability #inner }, bounds))
            }
            _ => None,
        }
    }

    let span = annotation.span();
    match walk(annotation, name) {
        Some((shape, bounds)) => Ok((shape, bounds)),
        None => Err(syn::Error::new(
            span,
            "the first parameter must be annotated with `impl Bound`, or any number of references \
             around one, as in `&impl Bound`, `&mut impl Bound` or `&mut &impl Bound`. Several \
             bounds behind a reference need parentheses, as in `&(impl Display + Clone)`",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::render;

    /// The message a signature is refused with, or a panic if it is accepted.
    fn refused(item: ItemFn) -> String {
        match Input::parse(TokenStream::new(), item) {
            Ok(_) => panic!("expected the signature to be refused"),
            Err(error) => error.to_string(),
        }
    }

    /// The message an option is refused with, or a panic if it is accepted.
    fn refused_option(args: TokenStream) -> String {
        let item = parse_quote!(
            pub fn describe(value: impl Display) {}
        );
        match Input::parse(args, item) {
            Ok(_) => panic!("expected the option to be refused"),
            Err(error) => error.to_string(),
        }
    }

    #[test]
    fn a_method_is_refused() {
        let message = refused(parse_quote!(
            fn describe(&self, value: impl Display) {}
        ));
        assert!(message.contains("free function"), "{message}");
    }

    #[test]
    fn a_signature_without_parameters_is_refused() {
        let message = refused(parse_quote!(
            fn describe() {}
        ));
        assert!(message.contains("first parameter"), "{message}");
    }

    #[test]
    fn an_unsafe_fn_is_refused() {
        let message = refused(parse_quote!(
            unsafe fn describe(value: impl Display) {}
        ));
        assert!(message.contains("`unsafe fn`"), "{message}");
    }

    #[test]
    fn an_explicit_abi_is_refused() {
        let message = refused(parse_quote!(
            extern "C" fn describe(value: impl Display) {}
        ));
        assert!(message.contains("ABI"), "{message}");
    }

    #[test]
    fn a_first_parameter_with_no_bounds_is_refused() {
        // Neither a type parameter the function declares nor an `impl Bound`, so
        // there is nothing to test.
        let message = refused(parse_quote!(
            fn describe(value: u8) {}
        ));
        assert!(message.contains("type parameter"), "{message}");
    }

    #[test]
    fn a_const_fn_is_accepted() {
        // Its constness is the function's own; only the macro's path is not const.
        assert!(
            Input::parse(
                TokenStream::new(),
                parse_quote!(
                    const fn describe(value: impl Display) {}
                ),
            )
            .is_ok()
        );
    }

    #[test]
    fn pub_is_refused_as_a_visibility() {
        let message = refused_option(quote!(vis = "pub"));
        assert!(message.contains("not currently supported"), "{message}");
        assert!(message.contains("pub(self)"), "{message}");
    }

    #[test]
    fn an_unknown_option_is_refused() {
        let message = refused_option(quote!(nonsense = "try_it"));
        assert!(message.contains("unknown option `nonsense`"), "{message}");
        assert!(message.contains("`vis` or `name`"), "{message}");
    }

    /// The options as `Input::parse` settles them, or a panic if it refuses.
    fn settled(args: TokenStream) -> (String, String) {
        let item = parse_quote!(
            pub fn describe(value: impl Display) {}
        );
        let input = Input::parse(args, item).expect("the options are accepted");
        (render(&input.macro_vis), input.macro_name.to_string())
    }

    #[test]
    fn the_macro_is_named_after_the_function_by_default() {
        assert_eq!(settled(TokenStream::new()).1, "try_describe");
    }

    #[test]
    fn a_name_may_be_given() {
        assert_eq!(settled(quote!(name = "probe_it")).1, "probe_it");
    }

    #[test]
    fn both_options_may_be_given_in_either_order() {
        let expected = ("pub (self)".to_owned(), "probe_it".to_owned());
        assert_eq!(
            settled(quote!(vis = "pub(self)", name = "probe_it")),
            expected
        );
        assert_eq!(
            settled(quote!(name = "probe_it", vis = "pub(self)")),
            expected
        );
    }

    #[test]
    fn a_trailing_comma_is_accepted() {
        assert_eq!(settled(quote!(name = "probe_it",)).1, "probe_it");
    }

    #[test]
    fn an_option_written_twice_is_refused() {
        let message = refused_option(quote!(name = "one", name = "two"));
        assert!(message.contains("`name` is written twice"), "{message}");

        let message = refused_option(quote!(vis = "pub(self)", vis = "pub(crate)"));
        assert!(message.contains("`vis` is written twice"), "{message}");
    }

    #[test]
    fn a_visibility_without_the_key_is_refused() {
        let message = refused_option(quote!(pub(crate)));
        assert!(message.contains(r#"`vis = ".."`"#), "{message}");
    }

    #[test]
    fn an_empty_visibility_is_refused() {
        let message = refused_option(quote!(vis = ""));
        assert!(message.contains("expected a visibility"), "{message}");
    }

    #[test]
    fn a_key_without_a_visibility_is_refused() {
        let message = refused_option(quote!(vis = "nonsense"));
        assert!(message.contains("expected a visibility"), "{message}");
    }

    #[test]
    fn a_pub_function_gets_a_crate_visible_macro() {
        let input = Input::parse(
            TokenStream::new(),
            parse_quote!(
                pub fn describe(value: impl Display) {}
            ),
        )
        .expect("the signature is accepted");
        assert_eq!(render(&input.macro_vis), "pub (crate)");
    }
}
