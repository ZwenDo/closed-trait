use closed_trait::sealed;

pub struct Bar;

// `'a` is declared but never used, so the generated impls quantify over a
// lifetime that constrains nothing.
#[sealed(for<'a> Bar)]
pub trait Foo {}

impl Foo for Bar {}

pub struct Held<T>(pub T);

// The same for a type, which rustc would otherwise refuse as an unconstrained
// parameter, pointing at the attribute rather than at the binder.
#[sealed(for<T> Held<i32>)]
pub trait Keep {}

impl Keep for Held<i32> {}

fn main() {}
