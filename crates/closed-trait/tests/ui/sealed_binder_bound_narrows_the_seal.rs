use closed_trait::sealed;
use std::fmt::Debug;

pub struct Plain;

// The binder permits `Store<T>` only where `T: Debug`, so an impl covering
// every `T` reaches instantiations the seal does not.
#[sealed(for<T: Debug> Plain: Store<T>)]
pub trait Store<T> {}

impl<T> Store<T> for Plain {}

fn main() {}
