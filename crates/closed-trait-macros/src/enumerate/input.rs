use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::ext::IdentExt;
use syn::parse::{ParseStream, Parser};
use syn::{
    Attribute, Error, GenericParam, Ident, ItemTrait, LitStr, Path, PathArguments, Result, Token,
    Type, parse_quote,
};

use crate::sealed::{self, SealedType};
use crate::util::{argument, lifetimes, mentions, name_of, render, snake_case};

const MATCH_ANY: &str = "match_any";
const NO_BRIDGE: &str = "no_bridge";
const SKIP: &str = "skip";

/// Which of the three enums a group of options is about.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Kind {
    Owned,
    Shared,
    Unique,
}

impl Kind {
    /// What the derived names end in.
    fn suffix(self) -> &'static str {
        match self {
            Kind::Owned => "",
            Kind::Shared => "Ref",
            Kind::Unique => "Mut",
        }
    }

    fn macro_suffix(self) -> &'static str {
        match self {
            Kind::Owned => "",
            Kind::Shared => "_ref",
            Kind::Unique => "_mut",
        }
    }
}

/// One of the three enums, as it was asked for.
pub(crate) struct Enumeration {
    pub(crate) ident: Ident,
    /// What to call its match macro, if `match_any` reached it.
    pub(crate) match_any: Option<Ident>,
    /// Whether the conversions written *on* this enum are to be left out.
    pub(crate) no_bridge: bool,
    /// Attributes to put on it verbatim. Never documentation: the enum's docs
    /// are generated.
    pub(crate) attrs: Vec<Attribute>,
}

/// A validated `#[enumerate]` invocation.
pub(crate) struct Input {
    /// The trait, still carrying its `#[sealed(..)]` attribute so that the
    /// attribute can expand after this one.
    pub(crate) item: ItemTrait,
    pub(crate) variants: Vec<Variant>,
    /// The enums' own parameter declarations, and the same names as the trait
    /// writes them in its supertrait bound.
    pub(crate) enum_params: Option<TokenStream>,
    pub(crate) enum_args: Option<TokenStream>,
    /// Each of the three, unless it was skipped.
    pub(crate) owned: Option<Enumeration>,
    pub(crate) shared: Option<Enumeration>,
    pub(crate) unique: Option<Enumeration>,
    /// Where the generated code should look for `Enumerable`. Defaults to
    /// `::closed_trait`, which is wrong for anyone who renamed the dependency
    /// or reaches the macros through a re-export.
    pub(crate) krate: Path,
}

pub(crate) struct Variant {
    pub(crate) ident: Ident,
    pub(crate) ty: Type,
    /// Parameters this variant's `Enumerable` impl declares.
    pub(crate) impl_params: Option<TokenStream>,
    /// Arguments it gives the enum, which the annotation may fix.
    pub(crate) enum_args: Option<TokenStream>,
}

impl Input {
    pub(crate) fn parse(args: TokenStream, item: ItemTrait) -> Result<Self> {
        let args = parse_args(args)?;

        let entries = sealed_types(&item)?;
        let shared_params = enum_parameters(&item, &entries);
        let variants = entries
            .iter()
            .map(|entry| variant(entry, &item, &shared_params))
            .collect::<Result<Vec<_>>>()?;

        let declarations = shared_params.iter().map(|param| quote!(#param));
        let arguments = shared_params.iter().map(argument);

        duplicate_variants(&variants)?;

        let enum_ident = args
            .grouped
            .name
            .clone()
            .unwrap_or_else(|| format_ident!("Any{}", item.ident));
        duplicate_conversions(&variants, &enum_ident)?;

        let owned = args.resolve(Kind::Owned, &item);
        let shared = args.resolve(Kind::Shared, &item);
        let unique = args.resolve(Kind::Unique, &item);

        if owned.is_none() && shared.is_none() && unique.is_none() {
            return Err(Error::new(
                Span::call_site(),
                "every enum was skipped, which leaves `#[enumerate]` nothing to generate",
            ));
        }

        // A match macro has one arm per variant, so an entry pinned to one
        // instantiation of a generic trait has no arm it could belong to.
        let wants_macro = [&owned, &shared, &unique]
            .into_iter()
            .flatten()
            .any(|enumeration| enumeration.match_any.is_some());
        if wants_macro {
            dispatchable(&item, &entries, &shared_params)?;
        }

        Ok(Input {
            enum_params: (!shared_params.is_empty()).then(|| quote!(<#(#declarations),*>)),
            enum_args: (!shared_params.is_empty()).then(|| quote!(<#(#arguments),*>)),
            owned,
            shared,
            unique,
            item,
            variants,
            krate: args.krate.unwrap_or_else(|| parse_quote!(::closed_trait)),
        })
    }
}

/// Options for one enum, either written in a group or applying to all three.
#[derive(Default, Clone)]
struct Options {
    skip: bool,
    name: Option<Ident>,
    /// `None` when `match_any` was never asked for, `Some(None)` when it was
    /// asked for without a name.
    match_any: Option<Option<Ident>>,
    /// The span it was written at, so a `ref(no_bridge)` can be refused where
    /// it stands.
    no_bridge: Option<Span>,
    attrs: Option<Vec<Attribute>>,
}

impl Options {
    /// Specific options win over grouped ones, field by field.
    fn over(&self, grouped: &Options) -> Options {
        Options {
            skip: self.skip,
            name: self.name.clone(),
            match_any: self.match_any.clone().or_else(|| grouped.match_any.clone()),
            no_bridge: self.no_bridge.or(grouped.no_bridge),
            attrs: self.attrs.clone(),
        }
    }
}

#[derive(Default)]
struct Args {
    /// Written bare, and so applying to all three unless overridden.
    grouped: Options,
    owned: Options,
    shared: Options,
    unique: Options,
    krate: Option<Path>,
}

impl Args {
    fn specific(&self, kind: Kind) -> &Options {
        match kind {
            Kind::Owned => &self.owned,
            Kind::Shared => &self.shared,
            Kind::Unique => &self.unique,
        }
    }

    /// One enum's settled options, or `None` if it was skipped.
    fn resolve(&self, kind: Kind, item: &ItemTrait) -> Option<Enumeration> {
        let options = self.specific(kind).over(&self.grouped);
        if options.skip {
            return None;
        }

        // A grouped `name` is a base that each kind extends; a specific one is
        // the name itself.
        let ident = options.name.clone().unwrap_or_else(|| {
            let base = self
                .grouped
                .name
                .clone()
                .unwrap_or_else(|| format_ident!("Any{}", item.ident));
            format_ident!("{base}{}", kind.suffix(), span = base.span())
        });

        // As with `name`, a specific one is the name itself while a grouped
        // one is a base that each kind extends.
        let match_any = options.match_any.map(|_| {
            if let Some(Some(named)) = &self.specific(kind).match_any {
                return named.clone();
            }
            let base = self
                .grouped
                .match_any
                .clone()
                .flatten()
                .unwrap_or_else(|| format_ident!("match_any_{}", snake_case(&item.ident)));
            format_ident!("{base}{}", kind.macro_suffix(), span = base.span())
        });

        Some(Enumeration {
            ident,
            match_any,
            no_bridge: options.no_bridge.is_some(),
            attrs: options.attrs.unwrap_or_default(),
        })
    }
}

/// The grouped options, and the `owned(..)` / `ref(..)` / `mut(..)` groups.
fn parse_args(args: TokenStream) -> Result<Args> {
    let mut parsed = Args::default();
    if args.is_empty() {
        return Ok(parsed);
    }

    let parser = |stream: ParseStream| -> Result<()> {
        while !stream.is_empty() {
            // `parse_any`, because `crate`, `ref` and `mut` are all keywords
            // and a plain `Ident` parse would reject them as option names.
            let key = Ident::parse_any(stream)?;
            let name = key.to_string();

            match name.as_str() {
                // A group: the same options, but only for one of the three.
                "owned" | "ref" | "mut" => {
                    let inner;
                    syn::parenthesized!(inner in stream);
                    let target = match name.as_str() {
                        "owned" => &mut parsed.owned,
                        "ref" => &mut parsed.shared,
                        _ => &mut parsed.unique,
                    };
                    parse_options(&inner, target, true)?;
                }
                // A string for the same reason `attrs` is one: it delimits the
                // path, so a malformed value says so instead of derailing the
                // rest of the list.
                "crate" => {
                    stream.parse::<Token![=]>()?;
                    if !stream.peek(LitStr) {
                        return Err(
                            stream.error(r#"expected a string, as in `crate = "::my_reexport"`"#)
                        );
                    }
                    let literal = stream.parse::<LitStr>()?;
                    let value = literal.parse::<Path>().map_err(|_| {
                        Error::new_spanned(&literal, "expected a path to the `closed-trait` crate")
                    })?;
                    if parsed.krate.replace(value).is_some() {
                        return Err(Error::new_spanned(&key, "duplicate `crate` option"));
                    }
                }
                _ => option(&key, stream, &mut parsed.grouped, false)?,
            }

            if stream.is_empty() {
                break;
            }
            stream.parse::<Token![,]>()?;
        }
        Ok(())
    };
    parser.parse2(args)?;

    // Nothing is written on the shared enum, so asking to leave it out is a
    // mistake rather than a no-op. Only a `ref(..)` group is refused; a bare
    // `no_bridge` reaching it through the grouped options is not.
    if let Some(span) = parsed.shared.no_bridge {
        return Err(Error::new(
            span,
            "`no_bridge` has nothing to leave out here: no conversion is written on the \
             borrowing enum's shared form.\nWrite it on `owned` to drop `as_ref` and `as_mut` \
             from the owned enum, or on `mut` to drop the reborrowing `as_ref`",
        ));
    }

    Ok(parsed)
}

/// A comma separated list of options inside a group.
fn parse_options(stream: ParseStream, options: &mut Options, in_group: bool) -> Result<()> {
    while !stream.is_empty() {
        let key = Ident::parse_any(stream)?;
        option(&key, stream, options, in_group)?;

        if stream.is_empty() {
            break;
        }
        stream.parse::<Token![,]>()?;
    }
    Ok(())
}

/// One option, wherever it was written.
fn option(key: &Ident, stream: ParseStream, options: &mut Options, in_group: bool) -> Result<()> {
    match key.to_string().as_str() {
        SKIP if in_group => options.skip = true,
        MATCH_ANY => {
            let named = if stream.peek(syn::token::Paren) {
                let inner;
                syn::parenthesized!(inner in stream);
                Some(inner.parse::<Ident>()?)
            } else {
                None
            };
            options.match_any = Some(named);
        }
        NO_BRIDGE => options.no_bridge = Some(key.span()),
        // A bare identifier: it names an item rather than carrying syntax that
        // needs delimiting.
        "name" => {
            stream.parse::<Token![=]>()?;
            let value = stream.parse::<Ident>()?;
            if options.name.replace(value).is_some() {
                return Err(Error::new_spanned(key, "duplicate `name` option"));
            }
        }
        // Only ever inside a group. What is valid differs between the three --
        // the shared enum already derives `Copy`, the unique one cannot derive
        // `Clone` at all -- so spreading one spelling across them would be a
        // trap rather than a convenience.
        "attrs" if !in_group => {
            return Err(Error::new_spanned(
                key,
                "`attrs` applies to one enum at a time, as in `owned(attrs = \"..\")`",
            ));
        }
        // A string, so the `#[..]` inside is unambiguous to both the parser and
        // to tooling. `parse_with` keeps error spans inside the literal rather
        // than on the attribute as a whole.
        "attrs" => {
            stream.parse::<Token![=]>()?;
            if !stream.peek(LitStr) {
                return Err(stream.error(
                    r##"expected a string of attributes, as in `attrs = "#[derive(Debug)]"`"##,
                ));
            }
            let literal = stream.parse::<LitStr>()?;
            let attrs = literal.parse_with(Attribute::parse_outer)?;
            // `///` desugars to `#[doc]`, so this catches both spellings.
            if let Some(doc) = attrs.iter().find(|attr| attr.path().is_ident("doc")) {
                return Err(Error::new_spanned(
                    doc,
                    "`attrs` cannot document the enum: its documentation is generated \
                     and is the same for every sealed trait",
                ));
            }
            options.attrs.get_or_insert_default().extend(attrs);
        }
        unknown => {
            let where_ = if in_group {
                "expected `skip`, `name`, `match_any`, `no_bridge` or `attrs`"
            } else {
                "expected `owned`, `ref`, `mut`, `name`, `match_any`, `no_bridge` or `crate`"
            };
            return Err(Error::new_spanned(
                key,
                format!("unknown option `{unknown}`, {where_}"),
            ));
        }
    }
    Ok(())
}

/// `#[enumerate]` has to sit above it.
fn sealed_types(item: &ItemTrait) -> Result<Vec<SealedType>> {
    // Other crates export a `sealed` attribute too, so a qualified path ending
    // in `sealed` is only a candidate. The bare spelling is unambiguous and
    // wins outright; otherwise take the first candidate whose arguments
    // actually parse as a type list, and let a lone candidate report its own
    // error rather than being passed over.
    let candidates: Vec<_> = item
        .attrs
        .iter()
        .filter(|attr| {
            attr.path()
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "sealed")
        })
        .collect();

    let bare = candidates
        .iter()
        .find(|attr| attr.path().is_ident("sealed"));
    let chosen = match (bare, candidates.as_slice()) {
        (Some(attr), _) => Some(*attr),
        (None, [only]) => Some(*only),
        (None, several) => several
            .iter()
            .copied()
            .find(|attr| parse_sealed(attr).is_ok()),
    };

    let attr = chosen.ok_or_else(|| {
        Error::new_spanned(
            &item.ident,
            "`#[enumerate]` needs a `#[sealed(..)]` attribute written below it, \
             to know which types the enum should hold",
        )
    })?;

    parse_sealed(attr)
}

fn parse_sealed(attr: &Attribute) -> Result<Vec<SealedType>> {
    Ok(sealed::Args::parse(attr.meta.require_list()?.tokens.clone())?.types)
}

/// The enum's parameters: those of the trait that at least one entry names.
///
/// They can only come from the trait, because they have to be nameable in the
/// supertrait bound `Enumerable<AnyThing<..>>` that names the enum, and
/// nothing else is in scope there.
fn enum_parameters(item: &ItemTrait, entries: &[SealedType]) -> Vec<GenericParam> {
    item.generics
        .params
        .iter()
        .filter(|param| entries.iter().any(|entry| uses(&entry.ty, param)))
        .cloned()
        .collect()
}

fn uses(ty: &Type, param: &GenericParam) -> bool {
    match param {
        GenericParam::Lifetime(param) => lifetimes(ty).contains(&param.lifetime.ident),
        GenericParam::Type(param) => mentions(ty, &param.ident.to_string()),
        GenericParam::Const(param) => mentions(ty, &param.ident.to_string()),
    }
}

/// The variant is named after the type's last path segment, so
/// `crate::shapes::Circle` becomes `Circle`.
fn variant(entry: &SealedType, item: &ItemTrait, shared: &[GenericParam]) -> Result<Variant> {
    let ty = &entry.ty;

    // A binder introduces parameters the trait never declared, so the pin could
    // not name them. Same reason a lifetime the trait does not declare is out.
    if entry.binder.is_some() {
        return Err(Error::new_spanned(
            ty,
            format!(
                "`#[enumerate]` cannot hold `{}`: a `for<..>` type is generic over parameters \
                 `{}` does not declare, so the generated enum could not be named in its \
                 supertrait bound.\nEither drop the binder and declare the parameter on \
                 `{}` itself, or remove `#[enumerate]`",
                render(ty),
                item.ident,
                item.ident,
            ),
        ));
    }

    let declared: Vec<GenericParam> = item
        .generics
        .params
        .iter()
        .filter(|param| uses(ty, param))
        .cloned()
        .collect();

    let free: Vec<_> = lifetimes(ty)
        .into_iter()
        .filter(|name| {
            !item
                .generics
                .lifetimes()
                .any(|param| &param.lifetime.ident == name)
        })
        .collect();
    if let Some(lifetime) = free.first() {
        return Err(Error::new_spanned(
            ty,
            format!(
                "`#[enumerate]` cannot hold `{}`: `'{}` is not a parameter of `{}`, so the \
                 generated enum could not be named in its supertrait bound.\nDeclare `'{}` on \
                 `{}` itself, or remove `#[enumerate]`",
                render(ty),
                lifetime,
                item.ident,
                lifetime,
                item.ident,
            ),
        ));
    }

    let arguments = enum_arguments(entry, item, shared, &declared)?;
    let declarations = declared.iter().map(|param| quote!(#param));

    // An explicit `as Alias` names the variant; otherwise it is the type's last
    // path segment, which is why two entries can collide.
    let ident = match &entry.alias {
        Some(alias) => alias.clone(),
        None => {
            let Type::Path(path) = ty else {
                return Err(Error::new_spanned(
                    ty,
                    "`#[enumerate]` needs each sealed type to be a path so the variant can be \
                     named after it, or an explicit `as Name`",
                ));
            };
            path.path
                .segments
                .last()
                .map(|segment| segment.ident.clone())
                .ok_or_else(|| {
                    Error::new_spanned(ty, "expected a path with at least one segment")
                })?
        }
    };

    Ok(Variant {
        ident,
        ty: ty.clone(),
        impl_params: (!declared.is_empty()).then(|| quote!(<#(#declarations),*>)),
        enum_args: arguments,
    })
}

/// The arguments this entry gives the enum in its `Enumerable` impl.
///
/// Every one has to be determined by the entry's own type, since an impl cannot
/// carry a parameter its self type does not constrain. A type that names the
/// trait's parameters determines them directly; one that does not needs the
/// annotation to fix them.
fn enum_arguments(
    entry: &SealedType,
    item: &ItemTrait,
    shared: &[GenericParam],
    declared: &[GenericParam],
) -> Result<Option<TokenStream>> {
    if shared.is_empty() {
        return Ok(None);
    }

    let fixed = entry.instantiation.as_ref().map(|path| {
        let arguments = match path.segments.last().map(|segment| &segment.arguments) {
            Some(PathArguments::AngleBracketed(arguments)) => {
                arguments.args.iter().cloned().collect::<Vec<_>>()
            }
            _ => Vec::new(),
        };
        item.generics
            .params
            .iter()
            .zip(arguments)
            .map(|(param, argument)| (name_of(param), quote!(#argument)))
            .collect::<Vec<_>>()
    });

    let mut arguments = Vec::new();
    for param in shared {
        let name = name_of(param);
        let annotated = match &fixed {
            Some(fixed) => fixed
                .iter()
                .find(|(fixed, _)| fixed == &name)
                .map(|(_, tokens)| tokens.clone()),
            None => None,
        };

        let chosen = match annotated {
            Some(annotated) => annotated,
            None if declared.iter().any(|known| name_of(known) == name) => argument(param),
            None => {
                let ty = &entry.ty;
                let sample = match param {
                    GenericParam::Lifetime(_) => "'static".to_owned(),
                    _ => "..".to_owned(),
                };
                let arguments = item
                    .generics
                    .params
                    .iter()
                    .map(|other| {
                        if name_of(other) == name {
                            sample.clone()
                        } else {
                            name_of_argument(other)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(Error::new_spanned(
                    ty,
                    format!(
                        "`{ty}` does not name `{name}`, which the generated enum is generic \
                         over, so there is no single `Any{trait_}` it can turn into.\nSay how it \
                         instantiates the trait: `{ty}: {trait_}<{arguments}>`",
                        ty = render(ty),
                        trait_ = item.ident,
                    ),
                ));
            }
        };
        arguments.push(chosen);
    }

    Ok(Some(quote!(<#(#arguments),*>)))
}

fn name_of_argument(param: &GenericParam) -> String {
    match param {
        GenericParam::Lifetime(param) => format!("'{}", param.lifetime.ident),
        other => name_of(other),
    }
}

/// Checks that the match can actually cover every entry.
///
/// The macro has one arm per variant of the enum at its own instantiation, so
/// every entry has to be one of them. Two things break that: an entry that
/// implements the trait at one fixed instantiation, and a trait parameter no
/// entry names, which would leave the bound with nothing to put in its place.
fn dispatchable(item: &ItemTrait, entries: &[SealedType], shared: &[GenericParam]) -> Result<()> {
    if let Some(unused) = item
        .generics
        .params
        .iter()
        .find(|param| !shared.iter().any(|used| name_of(used) == name_of(param)))
    {
        return Err(Error::new_spanned(
            unused,
            format!(
                "`#[enumerate({MATCH_ANY})]` needs every parameter of `{}` to appear in the \
                 sealed types, and `{}` appears in none of them, so the match has nothing to \
                 name.\nUse it in a sealed type, or drop the option",
                item.ident,
                name_of(unused),
            ),
        ));
    }

    if let Some(entry) = entries.iter().find(|entry| entry.instantiation.is_some()) {
        let instantiation = entry.instantiation.as_ref().expect("just matched");
        return Err(Error::new_spanned(
            &entry.ty,
            format!(
                "`#[enumerate({MATCH_ANY})]` cannot match `{ty}`: it implements `{had}`, not \
                 `{want}`, so it is not a variant of every `{want}`.\nDrop the `{MATCH_ANY}` \
                 option, or make `{ty}` generic over the same parameters as `{trait_}`",
                ty = render(&entry.ty),
                had = render(instantiation),
                want = render(&trait_bound(item, shared)),
                trait_ = item.ident,
            ),
        ));
    }

    Ok(())
}

/// The trait at the enum's own instantiation: `Shape`, or `Store<T>`.
pub(crate) fn trait_bound(item: &ItemTrait, shared: &[GenericParam]) -> TokenStream {
    let ident = &item.ident;
    let arguments = shared.iter().map(argument);
    let arguments = (!shared.is_empty()).then(|| quote!(<#(#arguments),*>));
    quote!(#ident #arguments)
}

/// Two entries mapping the same type into the same enum would each implement
/// `Enumerable` for it, leaving `into_enum` with two answers.
///
/// Sharing a type is otherwise fine: entries that pin different arguments land
/// in different enum types, so `Plain: Store<i32>` and `Plain: Store<f64>` can
/// coexist as long as the enum stays generic, which needs some entry to name
/// the parameter rather than fixing it.
fn duplicate_conversions(variants: &[Variant], enum_ident: &Ident) -> Result<()> {
    let key = |variant: &Variant| {
        let ty = &variant.ty;
        let args = &variant.enum_args;
        render(&quote!(#ty #args))
    };

    for (index, variant) in variants.iter().enumerate() {
        if variants[..index]
            .iter()
            .any(|earlier| key(earlier) == key(variant))
        {
            let ty = render(&variant.ty);
            let args = variant.enum_args.as_ref().map(render).unwrap_or_default();
            return Err(Error::new_spanned(
                &variant.ty,
                format!(
                    "`{ty}` is listed twice for the same `{enum_ident}{args}`, so `into_enum` \
                     would have two answers.\nEntries may share a type only when they pin \
                     different arguments, which needs the enum to stay generic — some entry has \
                     to name the parameter rather than fixing it"
                ),
            ));
        }
    }
    Ok(())
}

/// Two entries whose last path segment matches would produce one variant name
/// twice, which rustc reports against the generated enum rather than the list.
fn duplicate_variants(variants: &[Variant]) -> Result<()> {
    for (index, variant) in variants.iter().enumerate() {
        if let Some(earlier) = variants[..index]
            .iter()
            .find(|earlier| earlier.ident == variant.ident)
        {
            return Err(Error::new_spanned(
                &variant.ty,
                format!(
                    "`{}` and `{}` would both become the `{}` variant, since a variant is named \
                     after the type's last path segment.\nGive one an explicit name: \
                     `{} as SomeName`",
                    render(&earlier.ty),
                    render(&variant.ty),
                    variant.ident,
                    render(&variant.ty),
                ),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three enums as the attribute settles them, in owned/ref/mut order.
    fn resolved(attr: TokenStream) -> [Option<Enumeration>; 3] {
        let args = parse_args(attr).expect("the attribute parses");
        let item: ItemTrait = parse_quote!(
            pub trait Shape {}
        );
        [
            args.resolve(Kind::Owned, &item),
            args.resolve(Kind::Shared, &item),
            args.resolve(Kind::Unique, &item),
        ]
    }

    fn names(attr: TokenStream) -> Vec<Option<String>> {
        resolved(attr)
            .iter()
            .map(|kind| kind.as_ref().map(|kind| kind.ident.to_string()))
            .collect()
    }

    fn macros(attr: TokenStream) -> Vec<Option<String>> {
        resolved(attr)
            .iter()
            .map(|kind| {
                kind.as_ref()
                    .and_then(|kind| kind.match_any.as_ref())
                    .map(|name| name.to_string())
            })
            .collect()
    }

    fn bridges(attr: TokenStream) -> Vec<Option<bool>> {
        resolved(attr)
            .iter()
            .map(|kind| kind.as_ref().map(|kind| !kind.no_bridge))
            .collect()
    }

    #[test]
    fn bare_gives_all_three_and_no_macros() {
        assert_eq!(
            names(quote!()),
            vec![
                Some("AnyShape".to_owned()),
                Some("AnyShapeRef".to_owned()),
                Some("AnyShapeMut".to_owned()),
            ]
        );
        assert_eq!(macros(quote!()), vec![None, None, None]);
        assert_eq!(bridges(quote!()), vec![Some(true); 3]);
    }

    #[test]
    fn a_grouped_name_is_a_base_each_kind_extends() {
        assert_eq!(
            names(quote!(name = Shapes)),
            vec![
                Some("Shapes".to_owned()),
                Some("ShapesRef".to_owned()),
                Some("ShapesMut".to_owned()),
            ]
        );
    }

    #[test]
    fn a_specific_name_is_the_name_itself() {
        // and leaves the other two on the grouped base
        assert_eq!(
            names(quote!(name = Shapes, ref(name = View))),
            vec![
                Some("Shapes".to_owned()),
                Some("View".to_owned()),
                Some("ShapesMut".to_owned()),
            ]
        );
    }

    #[test]
    fn a_grouped_macro_name_is_a_base_too() {
        assert_eq!(
            macros(quote!(match_any)),
            vec![
                Some("match_any_shape".to_owned()),
                Some("match_any_shape_ref".to_owned()),
                Some("match_any_shape_mut".to_owned()),
            ]
        );
        assert_eq!(
            macros(quote!(match_any(walk))),
            vec![
                Some("walk".to_owned()),
                Some("walk_ref".to_owned()),
                Some("walk_mut".to_owned()),
            ]
        );
    }

    #[test]
    fn a_specific_macro_name_overrides_just_that_one() {
        assert_eq!(
            macros(quote!(match_any, mut(match_any(walk)))),
            vec![
                Some("match_any_shape".to_owned()),
                Some("match_any_shape_ref".to_owned()),
                Some("walk".to_owned()),
            ]
        );
    }

    #[test]
    fn a_macro_asked_for_in_one_group_only_reaches_that_one() {
        assert_eq!(
            macros(quote!(ref(match_any))),
            vec![None, Some("match_any_shape_ref".to_owned()), None]
        );
    }

    #[test]
    fn skip_drops_only_its_own_kind() {
        assert_eq!(
            names(quote!(ref(skip))),
            vec![
                Some("AnyShape".to_owned()),
                None,
                Some("AnyShapeMut".to_owned()),
            ]
        );
        assert_eq!(
            names(quote!(owned(skip), mut(skip))),
            vec![None, Some("AnyShapeRef".to_owned()), None]
        );
    }

    #[test]
    fn no_bridge_applies_to_the_enum_it_is_written_on() {
        assert_eq!(bridges(quote!(no_bridge)), vec![Some(false); 3]);
        assert_eq!(
            bridges(quote!(owned(no_bridge))),
            vec![Some(false), Some(true), Some(true)]
        );
        assert_eq!(
            bridges(quote!(mut(no_bridge))),
            vec![Some(true), Some(true), Some(false)]
        );
    }

    /// Nothing is written on the shared enum, so asking to leave it out is a
    /// mistake rather than a no-op.
    #[test]
    fn no_bridge_on_ref_is_refused() {
        assert!(refused(quote!(ref(no_bridge))).contains("nothing to leave out"));
    }

    /// The one option a group cannot take back: there is no positive spelling
    /// of `no_bridge`, so a grouped one stays on everywhere.
    #[test]
    fn a_grouped_no_bridge_cannot_be_undone_by_a_group() {
        assert_eq!(
            bridges(quote!(no_bridge, ref(name = View))),
            vec![Some(false); 3]
        );
    }

    #[test]
    fn attrs_reach_only_the_group_they_are_written_in() {
        let resolved = resolved(quote!(ref(attrs = "#[derive(Debug)]")));
        assert!(resolved[0].as_ref().expect("owned").attrs.is_empty());
        assert_eq!(resolved[1].as_ref().expect("ref").attrs.len(), 1);
        assert!(resolved[2].as_ref().expect("mut").attrs.is_empty());
    }

    /// `Args` has no `Debug`, syn's own impls being behind a feature, so the
    /// error comes out by hand rather than through `expect_err`.
    fn refused(attr: TokenStream) -> String {
        match parse_args(attr) {
            Err(error) => error.to_string(),
            Ok(_) => panic!("expected the attribute to be refused"),
        }
    }

    #[test]
    fn bare_attrs_is_refused() {
        assert!(refused(quote!(attrs = "#[derive(Debug)]")).contains("one enum at a time"));
    }

    #[test]
    fn skip_is_refused_outside_a_group() {
        assert!(refused(quote!(skip)).contains("unknown option `skip`"));
    }

    #[test]
    fn a_repeated_name_is_refused() {
        assert!(refused(quote!(name = A, name = B)).contains("duplicate `name`"));
        assert!(refused(quote!(crate = "::a", crate = "::b")).contains("duplicate `crate`"));
    }
}
