use proc_macro2::{TokenStream, TokenTree};
use quote::{ToTokens, quote};
use syn::ext::IdentExt;
use syn::{GenericParam, Ident};

/// Whether `tokens` name the identifier anywhere, including nested inside
/// generic arguments.
pub(crate) fn mentions(tokens: &impl ToTokens, name: &str) -> bool {
    fn walk(tokens: TokenStream, name: &str) -> bool {
        tokens.into_iter().any(|token| match token {
            TokenTree::Ident(ident) => ident == name,
            TokenTree::Group(group) => walk(group.stream(), name),
            _ => false,
        })
    }

    walk(tokens.to_token_stream(), name)
}

/// Every lifetime named in `tokens`, in order and without duplicates.
///
/// A lifetime reaches the token stream as an apostrophe followed by an
/// identifier, so both are needed to tell `Foo<'src>` from a type merely called
/// `src`. `'static` and `'_` are left out: neither can be declared on an impl.
pub(crate) fn lifetimes(tokens: &impl ToTokens) -> Vec<Ident> {
    fn walk(tokens: TokenStream, found: &mut Vec<Ident>) {
        let mut apostrophe = false;

        for token in tokens {
            match token {
                TokenTree::Group(group) => {
                    walk(group.stream(), found);
                    apostrophe = false;
                }
                TokenTree::Punct(punct) => apostrophe = punct.as_char() == '\'',
                TokenTree::Ident(ident) => {
                    if apostrophe && ident != "static" && ident != "_" && !found.contains(&ident) {
                        found.push(ident);
                    }
                    apostrophe = false;
                }
                TokenTree::Literal(_) => apostrophe = false,
            }
        }
    }

    let mut found = Vec::new();
    walk(tokens.to_token_stream(), &mut found);
    found
}

/// Rewrites every name in `tokens` that `renames` has an entry for.
///
/// Lifetimes are keyed with their apostrophe, so `'a` and a type parameter `a`
/// are told apart. Only whole idents match, leaving `SomeT` alone when `T` is
/// renamed, and the walk descends into groups so nested arguments are covered.
pub(crate) fn rename(tokens: &impl ToTokens, renames: &[(String, TokenStream)]) -> TokenStream {
    fn find<'a>(renames: &'a [(String, TokenStream)], name: &str) -> Option<&'a TokenStream> {
        renames
            .iter()
            .find(|(from, _)| from == name)
            .map(|(_, to)| to)
    }

    fn walk(tokens: TokenStream, renames: &[(String, TokenStream)]) -> TokenStream {
        let mut out = TokenStream::new();
        // Held back until the next token says whether it opens a lifetime.
        let mut apostrophe: Option<TokenTree> = None;

        for token in tokens {
            match token {
                TokenTree::Ident(ident) => match apostrophe.take() {
                    Some(punct) => match find(renames, &format!("'{ident}")) {
                        Some(to) => out.extend(to.clone()),
                        None => {
                            out.extend([punct, TokenTree::Ident(ident)]);
                        }
                    },
                    None => match find(renames, &ident.to_string()) {
                        Some(to) => out.extend(to.clone()),
                        None => out.extend([TokenTree::Ident(ident)]),
                    },
                },
                TokenTree::Punct(punct) if punct.as_char() == '\'' => {
                    out.extend(apostrophe.take());
                    apostrophe = Some(TokenTree::Punct(punct));
                }
                TokenTree::Group(group) => {
                    out.extend(apostrophe.take());
                    let inner = walk(group.stream(), renames);
                    out.extend([TokenTree::Group(proc_macro2::Group::new(
                        group.delimiter(),
                        inner,
                    ))]);
                }
                other => {
                    out.extend(apostrophe.take());
                    out.extend([other]);
                }
            }
        }
        out.extend(apostrophe);
        out
    }

    walk(tokens.to_token_stream(), renames)
}

/// Renders tokens the way a person would write them.
///
/// `TokenStream`'s own `to_string` separates every token, so a type comes out
/// as `Boxed < T >`; error messages quote types back at the reader, and that
/// spelling is distracting.
/// Whether an attribute is plausibly one of this crate's, by the spellings it is
/// usually reached under: bare, or qualified with the crate exporting it.
///
/// It is a guess, and cannot be anything else: an attribute macro is never told
/// the path it was invoked under, so it has no name of its own to compare
/// against. Every spelling here can name something else -- a bare one under
/// `use other::sealed`, a qualified one under a `use` or a renamed dependency --
/// and an import under a different name is missed entirely. What it costs is
/// bounded either way: a miss leaves rustc to report the duplicate items, and a
/// false positive is escaped by qualifying the other crate's attribute.
pub(crate) fn ours(attr: &syn::Attribute, name: &str) -> bool {
    let path = attr.path();
    if path.is_ident(name) {
        return true;
    }
    path.segments.last().is_some_and(|last| last.ident == name)
        && path.segments.first().is_some_and(|first| {
            first.ident == "closed_trait" || first.ident == "closed_trait_macros"
        })
}

pub(crate) fn render(tokens: &impl ToTokens) -> String {
    tokens
        .to_token_stream()
        .to_string()
        .replace(" <", "<")
        .replace("< ", "<")
        .replace(" >", ">")
        .replace("> ", ">")
        .replace(" ,", ",")
        .replace(" ::", "::")
        .replace(":: ", "::")
        // After the `::` pair above, so a path separator is already gone and
        // only a bound's own colon is left.
        .replace(" :", ":")
        .replace("& ", "&")
}

/// A generic parameter's name, without the apostrophe a lifetime carries.
///
/// Used to match a parameter against a name written in an attribute, where the
/// two kinds are told apart by position rather than by spelling.
pub(crate) fn name_of(param: &GenericParam) -> String {
    match param {
        GenericParam::Lifetime(param) => param.lifetime.ident.to_string(),
        GenericParam::Type(param) => param.ident.to_string(),
        GenericParam::Const(param) => param.ident.to_string(),
    }
}

/// A generic parameter as it appears in argument position, dropping the bounds
/// its declaration carries.
pub(crate) fn argument(param: &GenericParam) -> TokenStream {
    match param {
        GenericParam::Lifetime(param) => {
            let lifetime = &param.lifetime;
            quote!(#lifetime)
        }
        GenericParam::Type(param) => {
            let ident = &param.ident;
            quote!(#ident)
        }
        GenericParam::Const(param) => {
            let ident = &param.ident;
            quote!(#ident)
        }
    }
}

/// A type's name as a value's, for deriving `match_any_shape` from `Shape`.
///
/// A run of capitals is left alone rather than split apart, so `HttpRequest`
/// becomes `http_request` while `HTTPRequest` becomes `httprequest`. The
/// second is not what anyone would write by hand, which is what the option's
/// own name is for.
pub(crate) fn snake_case(ident: &Ident) -> String {
    let name = ident.unraw().to_string();
    let characters: Vec<char> = name.chars().collect();
    let mut snake = String::new();

    for (index, character) in characters.iter().enumerate() {
        if character.is_uppercase() {
            let after_lowercase = index > 0 && !characters[index - 1].is_uppercase();
            if after_lowercase {
                snake.push('_');
            }
            snake.extend(character.to_lowercase());
        } else {
            snake.push(*character);
        }
    }

    snake
}

/// `base` unless something in `taken` already answers to it, then `base2`,
/// `base3`, and so on.
///
/// Generated items stand beside the ones a caller wrote, so a name chosen
/// blindly can collide with theirs -- and a generic parameter that collides is
/// refused at best and a silently shadowed type at worst.
pub(crate) fn fresh(taken: impl IntoIterator<Item = String>, base: &str) -> Ident {
    let taken: Vec<String> = taken.into_iter().collect();
    let span = proc_macro2::Span::call_site();
    if !taken.iter().any(|name| name == base) {
        return Ident::new(base, span);
    }
    let mut suffix = 2;
    loop {
        let candidate = format!("{base}{suffix}");
        if !taken.contains(&candidate) {
            return Ident::new(&candidate, span);
        }
        suffix += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::{Type, parse_quote};

    #[test]
    fn mentions_looks_inside_generic_arguments() {
        let ty: Type = parse_quote!(Wrapper<Vec<Self>>);
        assert!(mentions(&ty, "Self"));
        assert!(mentions(&ty, "Vec"));
        assert!(!mentions(&ty, "Other"));
    }

    #[test]
    fn lifetimes_are_in_order_without_repeats() {
        let ty: Type = parse_quote!(Foo<'b, 'a, 'b>);
        let found: Vec<String> = lifetimes(&ty).iter().map(Ident::to_string).collect();
        assert_eq!(found, ["b", "a"]);
    }

    #[test]
    fn lifetimes_skips_the_ones_no_impl_can_declare() {
        let ty: Type = parse_quote!(Foo<'static, '_, 'a>);
        let found: Vec<String> = lifetimes(&ty).iter().map(Ident::to_string).collect();
        assert_eq!(found, ["a"]);
    }

    #[test]
    fn lifetimes_tells_a_lifetime_from_a_type_of_the_same_name() {
        let ty: Type = parse_quote!(Foo<src, 'src>);
        let found: Vec<String> = lifetimes(&ty).iter().map(Ident::to_string).collect();
        assert_eq!(found, ["src"], "only the one behind an apostrophe");
    }

    #[test]
    fn render_writes_types_the_way_a_person_would() {
        let ty: Type = parse_quote!(a::Boxed<T>);
        assert_eq!(render(&ty), "a::Boxed<T>");

        let predicate: syn::WherePredicate = parse_quote!(Self: Clone);
        assert_eq!(render(&predicate), "Self: Clone");

        let ty: Type = parse_quote!(&'a [u8]);
        assert_eq!(render(&ty), "&'a [u8]");
    }

    #[test]
    fn snake_case_splits_on_the_capitals_a_person_would() {
        assert_eq!(snake_case(&parse_quote!(Shape)), "shape");
        assert_eq!(snake_case(&parse_quote!(HttpRequest)), "http_request");
        assert_eq!(snake_case(&parse_quote!(already_snake)), "already_snake");
    }

    /// A run of capitals is left alone rather than split apart, which is why
    /// the option carries a name of its own.
    #[test]
    fn snake_case_leaves_a_run_of_capitals_alone() {
        assert_eq!(snake_case(&parse_quote!(HTTPRequest)), "httprequest");
    }

    #[test]
    fn a_parameters_name_drops_the_apostrophe() {
        assert_eq!(name_of(&parse_quote!('a)), "a");
        assert_eq!(name_of(&parse_quote!(T: Clone)), "T");
        assert_eq!(name_of(&parse_quote!(const N: usize)), "N");
    }

    #[test]
    fn an_argument_drops_the_bounds() {
        assert_eq!(render(&argument(&parse_quote!(T: Clone + Send))), "T");
        assert_eq!(render(&argument(&parse_quote!('a))), "'a");
        assert_eq!(render(&argument(&parse_quote!(const N: usize))), "N");
    }

    #[test]
    fn fresh_takes_the_base_when_nothing_holds_it() {
        assert_eq!(fresh(Vec::new(), "S").to_string(), "S");
        assert_eq!(fresh(vec!["T".to_owned()], "S").to_string(), "S");
    }

    #[test]
    fn fresh_counts_past_every_name_already_held() {
        let taken = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<Vec<_>>();
        assert_eq!(fresh(taken(&["S"]), "S").to_string(), "S2");
        assert_eq!(fresh(taken(&["S", "S2"]), "S").to_string(), "S3");
        // Gaps are not filled: it counts up from the base rather than hunting.
        assert_eq!(fresh(taken(&["S", "S3"]), "S").to_string(), "S2");
    }

    /// `rename` applied to a type, rendered back the way a person writes it.
    fn renamed(ty: TokenStream, renames: &[(&str, &str)]) -> String {
        let renames: Vec<(String, TokenStream)> = renames
            .iter()
            .map(|(from, to)| {
                let to: TokenStream = to.parse().expect("the replacement parses");
                ((*from).to_owned(), to)
            })
            .collect();
        render(&rename(&ty, &renames))
    }

    #[test]
    fn rename_replaces_whole_idents_only() {
        assert_eq!(renamed(quote!(Boxed<U>), &[("U", "T")]), "Boxed<T>");
        // `SomeU` merely contains the name.
        assert_eq!(renamed(quote!(Boxed<SomeU>), &[("U", "T")]), "Boxed<SomeU>");
    }

    #[test]
    fn rename_descends_into_nested_arguments() {
        assert_eq!(
            renamed(quote!(Boxed<Vec<(U, u8)>>), &[("U", "T")]),
            "Boxed<Vec<(T, u8)>>"
        );
    }

    #[test]
    fn rename_tells_a_lifetime_from_a_type_of_the_same_name() {
        // `'a` and a type parameter `a` are different names.
        assert_eq!(
            renamed(quote!(Slice<'a, a>), &[("'a", "'b")]),
            "Slice<'b, a>"
        );
        assert_eq!(renamed(quote!(Slice<'a, a>), &[("a", "T")]), "Slice<'a, T>");
    }

    #[test]
    fn rename_leaves_everything_else_alone() {
        assert_eq!(renamed(quote!(Plain), &[("U", "T")]), "Plain");
        assert_eq!(renamed(quote!(Boxed<U>), &[]), "Boxed<U>");
        // A lifetime with no rename keeps its apostrophe.
        assert_eq!(renamed(quote!(Slice<'a>), &[("U", "T")]), "Slice<'a>");
    }
}
