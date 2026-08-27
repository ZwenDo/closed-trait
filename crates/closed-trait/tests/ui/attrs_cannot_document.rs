use closed_trait::{enumerate, sealed};

pub struct Square;

#[enumerate(attrs = "/// Hand-written docs")]
#[sealed(Square)]
pub trait Shape {}

impl Shape for Square {}

fn main() {}
