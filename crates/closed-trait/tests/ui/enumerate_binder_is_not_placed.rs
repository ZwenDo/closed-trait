use closed_trait::{enumerate, sealed};

pub struct Boxed<U>(pub U);

// `U` is bound, but the instantiation pins `i32`, so nothing says which of
// `Store`'s parameters `U` stands for.
#[enumerate]
#[sealed(for<U> Boxed<U>: Store<i32>)]
pub trait Store<T> {}

impl<U> Store<i32> for Boxed<U> {}

fn main() {}
