use closed_trait::{enumerate, sealed};

pub struct Plain;
pub struct Boxed<T>(pub T);

#[enumerate(match_any)]
#[sealed(Plain: Store<i32>, for<T> Boxed<T>: Store<T>)]
pub trait Store<T> {}

impl Store<i32> for Plain {}
impl<T> Store<T> for Boxed<T> {}

fn main() {}
