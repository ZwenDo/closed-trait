use closed_trait::{enumerate, sealed};

pub struct Square;

// Each one generates the three enums, so the second collides with the first.
#[enumerate]
#[enumerate]
#[sealed(Square)]
pub trait Shape {}

impl Shape for Square {}

fn main() {}
