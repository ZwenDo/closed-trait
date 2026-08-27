use closed_trait::sealed;

pub struct Plain;

// `Az` is not the trait being sealed.
#[sealed(Plain: Az<i32>)]
pub trait Foo<T> {}

impl Foo<i32> for Plain {}

fn main() {}
