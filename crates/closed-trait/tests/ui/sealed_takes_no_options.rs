use closed_trait::sealed;

pub struct Square;

#[sealed(Square, unimplemented = "gone")]
pub trait Shape {}

impl Shape for Square {}

fn main() {}
