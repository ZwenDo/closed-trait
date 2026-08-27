use crate::sealed::expand::expand;
use crate::sealed::input::Input;
use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemTrait, parse_macro_input};

mod expand;
mod input;

pub(crate) use input::{Args, SealedType};

pub(crate) fn sealed(args: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemTrait);
    let original = item.clone();
    match Input::parse(args.into(), item) {
        Ok(input) => expand(input).into(),
        Err(error) => {
            let error = error.to_compile_error();
            quote! { #error #original }.into()
        }
    }
}
