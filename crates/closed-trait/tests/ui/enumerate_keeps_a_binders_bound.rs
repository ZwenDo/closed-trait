use closed_trait::{enumerate, sealed};
use closed_trait::Enumerable;
use std::fmt::Debug;

pub struct Held<U>(pub U);

#[enumerate]
#[sealed(for<U: Debug> Held<U>: Keep<U>)]
pub trait Keep<T> {}

impl<U: Debug> Keep<U> for Held<U> {}

pub struct NotDebug;

// `Held<NotDebug>` does not implement `Keep<NotDebug>`, so it is not a permitted
// type and must not reach the enum.
fn main() {
    let _: AnyKeep<NotDebug> = Held(NotDebug).into_enum();
}
