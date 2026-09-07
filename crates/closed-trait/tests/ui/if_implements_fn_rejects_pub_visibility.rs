use closed_trait::if_implements_fn;
use std::fmt::Display;

#[if_implements_fn(vis = "pub")]
pub fn describe(value: impl Display) -> String {
    format!("{value}")
}

fn main() {}
