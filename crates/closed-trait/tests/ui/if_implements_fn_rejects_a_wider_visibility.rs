use closed_trait::if_implements_fn;
use std::fmt::Display;

// The macro expands to a call to `describe`, so it cannot be reachable from
// where the function is not.
#[if_implements_fn(vis = "pub(crate)")]
fn describe(value: impl Display) -> String {
    format!("{value}")
}

fn main() {}
