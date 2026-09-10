use closed_trait::if_implements_fn;
use std::fmt::Display;

#[if_implements_fn]
extern "C" fn describe(value: impl Display) -> String {
    format!("{value}")
}

fn main() {}
