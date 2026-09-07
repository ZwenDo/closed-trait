mod expand;
mod input;

use crate::implements::expand::expand;
use crate::implements::input::Input;
use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

pub(crate) fn if_implements_attribute(args: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemFn);
    let original = item.clone();
    match Input::parse(args.into(), item) {
        Ok(input) => expand(input).into(),
        Err(error) => {
            let error = error.to_compile_error();
            quote! { #error #original }.into()
        }
    }
}
