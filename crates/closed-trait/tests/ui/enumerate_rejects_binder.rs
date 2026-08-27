use closed_trait::{enumerate, sealed};

pub struct Boxed<T>(pub T);

#[enumerate]
#[sealed(for<T> Boxed<T>)]
pub trait Shape {}

impl<T> Shape for Boxed<T> {}

fn main() {}
