use closed_trait::sealed;

pub struct Plain;
pub struct Boxed<T>(pub T);

// `Plain` names none of the trait's parameters, so nothing could check that it
// implements `Store` at all.
#[sealed(Plain, Boxed<T>)]
pub trait Store<T> {}

impl Store<i32> for Plain {}
impl<T> Store<T> for Boxed<T> {}

fn main() {}
