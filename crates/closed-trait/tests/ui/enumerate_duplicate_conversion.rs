use closed_trait::{enumerate, sealed};

// The alias settles the variant name, but both entries still map `Plain` into
// the same non-generic `AnyFoo`, so `into_enum` would have two answers.
#[enumerate]
#[sealed(Plain: Foo<i32>, Plain as PlainF64: Foo<f64>)]
pub trait Foo<T> {}

pub struct Plain;
impl Foo<i32> for Plain {}
impl Foo<f64> for Plain {}

fn main() {}
