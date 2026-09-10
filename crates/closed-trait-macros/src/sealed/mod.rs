use crate::sealed::expand::expand;
use crate::sealed::input::Input;
use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemTrait, parse_macro_input};

use crate::util::{ours, render};

mod expand;
mod input;

pub(crate) use input::{Args, SealedType, needs_instantiation};

pub(crate) fn process(args: TokenStream, item: TokenStream) -> TokenStream {
    let mut item = parse_macro_input!(item as ItemTrait);

    // An attribute is handed the item with the ones below it still attached, so
    // a second `#[sealed(..)]` is visible from the first. Both lists are dropped
    // rather than one expanded: sealing the trait from either would refuse every
    // type the other listed, burying the one mistake under its consequences.
    if let Some(again) = item.attrs.iter().find(|attr| ours(attr, "sealed")) {
        let error = syn::Error::new_spanned(
            again,
            format!(
                "`#[{}(..)]` is written twice, and a trait is sealed to one list.\nWrite one \
                 attribute listing every permitted type",
                render(again.path()),
            ),
        )
        .to_compile_error();
        item.attrs.retain(|attr| !ours(attr, "sealed"));
        return quote! { #error #item }.into();
    }

    let original = item.clone();
    match Input::parse(args.into(), item) {
        Ok(input) => expand(input).into(),
        Err(error) => {
            let error = error.to_compile_error();
            quote! { #error #original }.into()
        }
    }
}
