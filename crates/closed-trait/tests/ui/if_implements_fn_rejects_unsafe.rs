use closed_trait::if_implements_fn;
use std::fmt::Display;

#[if_implements_fn]
unsafe fn describe(value: impl Display) -> String {
    format!("{value}")
}

fn main() {}
