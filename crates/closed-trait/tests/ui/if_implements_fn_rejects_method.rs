use closed_trait::if_implements_fn;
use std::fmt::Display;

pub struct Printer;

impl Printer {
    #[if_implements_fn]
    fn describe(&self, value: impl Display) -> String {
        let _ = self;
        format!("{value}")
    }
}

fn main() {}
