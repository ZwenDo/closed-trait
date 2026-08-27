use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Lifetime, parse_quote};

use super::input::{Enumeration, Input, Variant};
use crate::util::render;

pub(crate) fn expand(input: Input) -> TokenStream {
    let Input {
        mut item,
        variants,
        enum_params,
        enum_args,
        owned,
        shared,
        unique,
        krate,
    } = input;

    // A lifetime the trait has not already spoken for, since the borrowing
    // enums need one of their own.
    let lifetime = free_lifetime(&item);

    // Naming an enum in the supertrait bound is what lets callers reach it
    // through the trait alone, and keeps `dyn Trait` usable. Each is a type
    // parameter rather than an associated type so that one type can belong to
    // several sealed traits.
    if let Some(enumeration) = &owned {
        let ident = &enumeration.ident;
        item.supertraits
            .push(parse_quote!(#krate::Enumerable<#ident #enum_args>));
    }
    // Higher-ranked, so that `S: Shape` alone yields the enum: the lifetime
    // belongs to the bound rather than to the caller.
    for (borrow, enumeration) in [(Borrow::Shared, &shared), (Borrow::Unique, &unique)] {
        if let Some(enumeration) = enumeration {
            let ident = &enumeration.ident;
            let args = with_lifetime(&lifetime, &enum_args);
            let lending = borrow.lending_trait();
            item.supertraits
                .push(parse_quote!(for<#lifetime> #krate::#lending<#lifetime, #ident #args>));
        }
    }

    // The enums carry the trait's visibility: they appear in the trait's own
    // supertrait bounds, so anything narrower is a private type in a public
    // interface.
    let vis = &item.vis;

    let owning = owned
        .as_ref()
        .map(|enumeration| owned_enum(enumeration, &enum_params, vis, &krate, &variants));

    let borrowing: Vec<TokenStream> = [(Borrow::Shared, &shared), (Borrow::Unique, &unique)]
        .into_iter()
        .filter_map(|(borrow, enumeration)| {
            Some(borrowing_enum(
                borrow,
                enumeration.as_ref()?,
                &enum_params,
                vis,
                &krate,
                &lifetime,
                &variants,
            ))
        })
        .collect();

    let bridges = bridges(
        owned.as_ref(),
        shared.as_ref(),
        unique.as_ref(),
        &enum_params,
        &enum_args,
        vis,
        &variants,
    );

    let macros: Vec<TokenStream> = [
        (None, &owned),
        (Some(Borrow::Shared), &shared),
        (Some(Borrow::Unique), &unique),
    ]
    .into_iter()
    .filter_map(|(borrow, enumeration)| {
        let enumeration = enumeration.as_ref()?;
        let name = enumeration.match_any.as_ref()?;
        Some(match_macro(
            name,
            vis,
            &enumeration.ident,
            &variants,
            borrow,
        ))
    })
    .collect();

    quote! {
        #owning

        #(#borrowing)*

        #(#bridges)*

        #(#macros)*

        #item
    }
}

/// The owned enum, holding each permitted type by value.
fn owned_enum(
    enumeration: &Enumeration,
    enum_params: &Option<TokenStream>,
    vis: &syn::Visibility,
    krate: &syn::Path,
    variants: &[Variant],
) -> TokenStream {
    let Enumeration { ident, attrs, .. } = enumeration;

    let fields = variants.iter().map(|variant| {
        let name = &variant.ident;
        let ty = &variant.ty;
        quote! { #name(#ty), }
    });

    let impls = variants.iter().map(|variant| {
        let name = &variant.ident;
        let ty = &variant.ty;
        let params = &variant.impl_params;
        let args = &variant.enum_args;
        quote! {
            #[automatically_derived]
            impl #params #krate::Enumerable<#ident #args> for #ty {
                #[inline]
                fn into_enum(self) -> #ident #args {
                    #ident::#name(self)
                }
            }

            // The conventional spelling of `into_enum`, so the enum composes
            // with anything that builds through `Into`.
            #[automatically_derived]
            impl #params ::core::convert::From<#ty> for #ident #args {
                #[inline]
                fn from(value: #ty) -> Self {
                    #ident::#name(value)
                }
            }
        }
    });

    let summary = "The types permitted to implement the trait, held by value.";
    let detail = "One variant per type listed in the `#[sealed(..)]` attribute on the trait, \
                  holding that type. Obtained from any implementor with `into_enum`.";

    quote! {
        #[doc = #summary]
        #[doc = ""]
        #[doc = #detail]
        // Variants are named after the types, so a primitive or any other
        // type not in upper camel case would have rustc suggesting a rename of
        // a name the caller never chose. `as Name` is there for anyone who
        // wants a different one.
        #[allow(non_camel_case_types)]
        #(#attrs)*
        #vis enum #ident #enum_params {
            #(#fields)*
        }

        #(#impls)*
    }
}

/// The conversions between the generated enums.
///
/// `no_bridge` on an enum leaves out the conversions written *on* it, which is
/// what every other per-enum option configures: `owned(no_bridge)` drops
/// `as_ref` and `as_mut` from the owned enum, `mut(no_bridge)` drops the
/// reborrowing `as_ref`.
fn bridges(
    owned: Option<&Enumeration>,
    shared: Option<&Enumeration>,
    unique: Option<&Enumeration>,
    enum_params: &Option<TokenStream>,
    enum_args: &Option<TokenStream>,
    vis: &syn::Visibility,
    variants: &[Variant],
) -> Vec<TokenStream> {
    let elided = elided(enum_args);
    let arms = |from: &syn::Ident, to: &syn::Ident| {
        let arms = variants.iter().map(|variant| {
            let name = &variant.ident;
            quote! { #from::#name(__inner) => #to::#name(__inner), }
        });
        quote! { match self { #(#arms)* } }
    };

    let mut bridges = Vec::new();

    if let Some(owned) = owned.filter(|owned| !owned.no_bridge) {
        let from = &owned.ident;
        let to_shared = shared.map(|to| {
            let to = &to.ident;
            let body = arms(from, to);
            let doc = format!("Borrows the held value, giving a [`{to}`].");
            quote! {
                #[doc = #doc]
                #[inline]
                #vis fn as_ref(&self) -> #to #elided { #body }
            }
        });
        let to_unique = unique.map(|to| {
            let to = &to.ident;
            let body = arms(from, to);
            let doc = format!("Borrows the held value mutably, giving a [`{to}`].");
            quote! {
                #[doc = #doc]
                #[inline]
                #vis fn as_mut(&mut self) -> #to #elided { #body }
            }
        });

        if to_shared.is_some() || to_unique.is_some() {
            bridges.push(quote! {
                #[automatically_derived]
                impl #enum_params #from #enum_args {
                    #to_shared
                    #to_unique
                }
            });
        }
    }

    // A unique borrow reborrows as a shared one, for the length of the borrow
    // of the enum rather than of what it holds.
    if let (Some(from), Some(to)) = (unique.filter(|from| !from.no_bridge), shared) {
        let params = with_lifetime(
            &Lifetime::new("'__a", proc_macro2::Span::call_site()),
            enum_params,
        );
        let args = with_lifetime(
            &Lifetime::new("'__a", proc_macro2::Span::call_site()),
            enum_args,
        );
        let from_ident = &from.ident;
        let to_ident = &to.ident;
        let body = arms(from_ident, to_ident);
        let doc = format!("Reborrows as a [`{to_ident}`], for as long as this is borrowed.");
        bridges.push(quote! {
            #[automatically_derived]
            impl #params #from_ident #args {
                #[doc = #doc]
                #[inline]
                #vis fn as_ref(&self) -> #to_ident #elided { #body }
            }
        });
    }

    bridges
}

/// `<T>` becomes `<'_, T>`, and nothing becomes `<'_>`.
fn elided(args: &Option<TokenStream>) -> TokenStream {
    match args {
        None => quote!(<'_>),
        Some(args) => {
            let text = args.to_string();
            let inner = text.trim().trim_start_matches('<').trim_end_matches('>');
            let inner: TokenStream = inner.parse().expect("re-parsing our own arguments");
            quote!(<'_, #inner>)
        }
    }
}

/// A lifetime name the trait has not declared.
fn free_lifetime(item: &syn::ItemTrait) -> Lifetime {
    let taken: Vec<String> = item
        .generics
        .lifetimes()
        .map(|param| param.lifetime.ident.to_string())
        .collect();

    let name = ["a", "r", "__ref"]
        .into_iter()
        .find(|candidate| !taken.iter().any(|used| used == candidate))
        .unwrap_or("__enumerate_ref");
    Lifetime::new(&format!("'{name}"), proc_macro2::Span::call_site())
}

/// `<T>` becomes `<'a, T>`, and nothing becomes `<'a>`.
fn with_lifetime(lifetime: &Lifetime, params: &Option<TokenStream>) -> TokenStream {
    match params {
        None => quote!(<#lifetime>),
        Some(params) => {
            let text = params.to_string();
            let inner = text.trim().trim_start_matches('<').trim_end_matches('>');
            let inner: TokenStream = inner.parse().expect("re-parsing our own parameters");
            quote!(<#lifetime, #inner>)
        }
    }
}

/// Which of the two borrowing enums is being generated.
///
/// Rust cannot abstract over `&` and `&mut`, so the two are separate types in
/// the generated API. They differ only in the reference they hold, though, so
/// everything about producing them factors through here.
#[derive(Clone, Copy)]
enum Borrow {
    Shared,
    Unique,
}

impl Borrow {
    /// The reference a variant holds.
    fn reference(self, lifetime: &Lifetime) -> TokenStream {
        match self {
            Borrow::Shared => quote!(&#lifetime),
            Borrow::Unique => quote!(&#lifetime mut),
        }
    }

    /// The receiver the lending method takes.
    fn receiver(self, lifetime: &Lifetime) -> TokenStream {
        match self {
            Borrow::Shared => quote!(&#lifetime self),
            Borrow::Unique => quote!(&#lifetime mut self),
        }
    }

    fn lending_trait(self) -> syn::Ident {
        match self {
            Borrow::Shared => format_ident!("EnumerableRef"),
            Borrow::Unique => format_ident!("EnumerableMut"),
        }
    }

    fn method(self) -> syn::Ident {
        match self {
            Borrow::Shared => format_ident!("as_enum_ref"),
            Borrow::Unique => format_ident!("as_enum_mut"),
        }
    }

    fn described(self) -> &'static str {
        match self {
            Borrow::Shared => "a shared reference to",
            Borrow::Unique => "a unique reference to",
        }
    }
}

/// A borrowing enum: one variant per permitted type, holding a reference.
///
/// `Enumerable` cannot reach either of these, `into_enum` taking `self` by
/// value, so a borrow has no route to the owned enum at all. They are also
/// cheaper to pass, being a pointer and a discriminant rather than as wide as
/// the largest permitted type.
#[allow(clippy::too_many_arguments)]
fn borrowing_enum(
    borrow: Borrow,
    enumeration: &Enumeration,
    enum_params: &Option<TokenStream>,
    vis: &syn::Visibility,
    krate: &syn::Path,
    lifetime: &Lifetime,
    variants: &[Variant],
) -> TokenStream {
    let Enumeration { ident, attrs, .. } = enumeration;
    let params = with_lifetime(lifetime, enum_params);
    let reference = borrow.reference(lifetime);
    let receiver = borrow.receiver(lifetime);
    let lending = borrow.lending_trait();
    let method = borrow.method();

    let fields = variants.iter().map(|variant| {
        let name = &variant.ident;
        let ty = &variant.ty;
        quote! { #name(#reference #ty), }
    });

    let impls = variants.iter().map(|variant| {
        let name = &variant.ident;
        let ty = &variant.ty;
        let impl_params = with_lifetime(lifetime, &variant.impl_params);
        let impl_args = with_lifetime(lifetime, &variant.enum_args);
        quote! {
            #[automatically_derived]
            impl #impl_params #krate::#lending<#lifetime, #ident #impl_args> for #ty {
                #[inline]
                fn #method(#receiver) -> #ident #impl_args {
                    #ident::#name(self)
                }
            }

            #[automatically_derived]
            impl #impl_params ::core::convert::From<#reference #ty> for #ident #impl_args {
                #[inline]
                fn from(value: #reference #ty) -> Self {
                    #ident::#name(value)
                }
            }
        }
    });

    // A unique reference is neither, so only the shared enum gets them.
    let copy = matches!(borrow, Borrow::Shared).then(|| {
        quote! { #[derive(::core::clone::Clone, ::core::marker::Copy)] }
    });

    let summary = format!(
        "The types permitted to implement the trait, held as {} each.",
        borrow.described(),
    );
    let detail = format!(
        "The same variants as the owned enum, each holding {} the type. Obtained from any \
         implementor with `{method}`, or from a reference to one with `From`.",
        borrow.described(),
    );

    quote! {
        #[doc = #summary]
        #[doc = ""]
        #[doc = #detail]
        #copy
        // Variants are named after the types, so a primitive or any other
        // type not in upper camel case would have rustc suggesting a rename of
        // a name the caller never chose. `as Name` is there for anyone who
        // wants a different one.
        #[allow(non_camel_case_types)]
        #(#attrs)*
        #vis enum #ident #params {
            #(#fields)*
        }

        #(#impls)*
    }
}

/// The macro that matches the enum, one arm per variant.
///
/// Rust has no generic closures, so a body that has to run against the
/// concrete type cannot be passed as a value. Pasting it into every arm is the
/// way to have one body and still know which type it has, and unlike a method
/// taking it, `return` and `?` in that body leave the enclosing function.
///
/// The binding has to be named by the caller: a `macro_rules!` one would be
/// invisible to the body, which is passed in and keeps its own hygiene.
fn match_macro(
    name: &syn::Ident,
    vis: &syn::Visibility,
    enum_ident: &syn::Ident,
    variants: &[Variant],
    borrow: Option<Borrow>,
) -> TokenStream {
    let arms = variants.iter().map(|variant| {
        let variant = &variant.ident;
        quote! { #enum_ident::#variant($binding) => $body, }
    });
    let rule = quote! {
        ($value:expr, $binding:pat => $body:expr) => {
            match $value {
                #(#arms)*
            }
        };
    };

    // Named after the types themselves, so the documentation says what the
    // binding actually is rather than describing it in the abstract.
    let mut named: Vec<String> = variants
        .iter()
        .take(4)
        .map(|variant| format!("`{}`", render(&variant.ty)))
        .collect();
    if variants.len() > 4 {
        named.push("…".to_owned());
    }
    let named = named.join(", ");

    let held = if borrow.is_some() { "borrows" } else { "holds" };
    let summary = format!("Runs one body against whichever type an `{enum_ident}` {held}.");
    // `text`, and not `ignore` or `no_run`: anything rustdoc still considers
    // Rust is compiled as a doctest of whichever crate this expands in, where
    // none of these names are in scope.
    let fence = "```ignore";
    // The borrowing enum already holds references, and is `Copy`, so there is
    // only ever one way to pass it.
    let (by_ref, by_mut, by_value) = if borrow.is_some() {
        (
            format!("{name}!(value, binding => body)"),
            String::new(),
            String::new(),
        )
    } else {
        (
            format!("{name}!(&value,     binding => body)"),
            format!("{name}!(&mut value, binding => body)"),
            format!("{name}!(value,      binding => body)"),
        )
    };
    let close = "```";

    let form = if let Some(borrow) = borrow {
        let described = borrow.described();
        format!(
            "Expands to a `match` over every variant, so the binding is the concrete type \
             rather than the enum: {described} {named}."
        )
    } else {
        format!(
            "Expands to a `match` over every variant, so the binding is the concrete type rather \
             than the enum. In the three forms above it is a shared reference to, a unique \
             reference to, or an owned {named}, and match ergonomics make it follow the value."
        )
    };
    let naming = "The binding is named at the call site rather than by the macro. One the macro \
                  introduced would be invisible to the body, since the body is passed in and \
                  keeps the hygiene of where it was written.";
    let detail = "This is a `match`, not a closure: `return` and `?` in the body leave the \
                  enclosing function, and the body may move anything it owns, since only one arm \
                  ever runs.";
    let cost = "The body is copied into every arm, so it is type-checked once per variant: a \
                long body costs compile time in proportion, and one mistake in it is reported \
                once per variant. Nesting one of these inside another squares the count. Moving \
                a big body into a generic function and calling that from the arm fixes both, \
                and costs nothing in code size, which was only ever the price of monomorphising \
                over the concrete type.";
    let scope = format!(
        "`{enum_ident}` and the trait have to be in scope where this is called, because a \
         `macro_rules!` body resolves paths at the call site."
    );
    let docs = quote! {
        #[doc = #summary]
        #[doc = ""]
        #[doc = #fence]
        #[doc = #by_ref]
        #[doc = #by_mut]
        #[doc = #by_value]
        #[doc = #close]
        #[doc = ""]
        #[doc = #form]
        #[doc = ""]
        #[doc = #naming]
        #[doc = ""]
        #[doc = #detail]
        #[doc = ""]
        #[doc = #cost]
        #[doc = ""]
        #[doc = #scope]
    };

    // A `macro_rules!` cannot leave the crate that defines it without
    // `#[macro_export]`, which always plants it in the crate root. That is
    // worth a hidden name at the root only when the trait is public enough for
    // another crate to reach it; anything narrower stays where it was written.
    if matches!(vis, syn::Visibility::Public(_)) {
        // The alias reaches the macro by textual scope rather than by `crate::`,
        // which a macro-expanded `macro_export` forbids (rust#52234).
        let exported = format_ident!("__sealed_{name}");
        quote! {
            // The documentation belongs on the definition, not on the alias:
            // an editor resolves the invocation to the `macro_rules!` and reads
            // it from there, and would find nothing on a bare re-export.
            // `#[doc(hidden)]` keeps the mangled name out of the rendered
            // documentation without detaching the docs from it.
            #docs
            #[doc(hidden)]
            #[macro_export]
            macro_rules! #exported {
                #rule
            }

            // Pulls those same docs onto the alias, which is the name callers
            // actually see, instead of a bare re-export line.
            #[doc(inline)]
            #vis use #exported as #name;
        }
    } else {
        quote! {
            #docs
            macro_rules! #name {
                #rule
            }

            // Path-addressable, so a module that cannot see the textual scope
            // can still reach it. Redundant within the defining module itself,
            // which is what the `allow` is for.
            #[allow(unused_imports)]
            #vis use #name;
        }
    }
}
