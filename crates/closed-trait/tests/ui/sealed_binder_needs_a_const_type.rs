use closed_trait::sealed;

pub struct Array<const N: usize>(pub [u8; N]);

// A const parameter needs its type, here as anywhere else.
#[sealed(for<const N> Array<N>)]
pub trait Shape {}

impl<const N: usize> Shape for Array<N> {}

fn main() {}
