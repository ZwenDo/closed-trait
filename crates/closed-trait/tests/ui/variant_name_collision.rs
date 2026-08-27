use closed_trait::{enumerate, sealed};

pub mod a { pub struct Foo; }
pub mod b { pub struct Foo; }

#[enumerate]
#[sealed(a::Foo, b::Foo)]
pub trait Shape {}

impl Shape for a::Foo {}
impl Shape for b::Foo {}

fn main() {}
