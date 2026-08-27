use closed_trait::{enumerate, sealed};

pub struct Square;

// No conversion is written on the shared enum, so there is nothing to leave out.
#[enumerate(ref(no_bridge))]
#[sealed(Square)]
pub trait Shape {}

impl Shape for Square {}

fn main() {}
