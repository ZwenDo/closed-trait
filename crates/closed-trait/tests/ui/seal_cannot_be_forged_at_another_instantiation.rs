use closed_trait::sealed;

pub struct Plain;

#[sealed(Plain: Foo<i32>)]
pub trait Foo<T> {}

impl Foo<i32> for Plain {}

// Beside the trait, so `__sealed_Foo` is nameable, and `Plain` is already
// sealed at `i32`. The marker carries the arguments, so that buys nothing.
impl __sealed_Foo::Sealed<f64> for Plain {}
impl Foo<f64> for Plain {}

fn main() {}
