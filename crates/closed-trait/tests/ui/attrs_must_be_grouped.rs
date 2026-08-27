use closed_trait::{enumerate, sealed};

pub struct Square;

#[enumerate(attrs = "#[derive(Debug)]")]
#[sealed(Square)]
pub trait Shape {}

impl Shape for Square {}

fn main() {}
