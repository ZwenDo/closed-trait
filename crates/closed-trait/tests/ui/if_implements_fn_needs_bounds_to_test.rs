use closed_trait::if_implements_fn;

#[if_implements_fn]
fn describe(value: u8) -> String {
    format!("{value}")
}

fn main() {}
