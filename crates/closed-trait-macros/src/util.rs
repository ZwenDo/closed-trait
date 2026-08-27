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

/// Renders tokens the way a person would write them.
///
/// `TokenStream`'s own `to_string` separates every token, so a type comes out
/// as `Boxed < T >`; error messages quote types back at the reader, and that
/// spelling is distracting.
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
}
