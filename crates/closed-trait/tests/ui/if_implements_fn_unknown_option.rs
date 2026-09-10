use closed_trait::if_implements_fn;
use std::fmt::Display;

#[if_implements_fn(nonsense = "try_it")]
fn describe(value: impl Display) -> String {
    format!("{value}")
}

fn main() {}
