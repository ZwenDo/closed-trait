use closed_trait::sealed;

pub struct Plain;

// The list permits `Plain` at `i32`, and says nothing about `f64`.
#[sealed(Plain: Foo<i32>)]
pub trait Foo<T> {}

impl Foo<i32> for Plain {}
impl Foo<f64> for Plain {}

fn main() {}
