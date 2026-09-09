use closed_trait::{enumerate, sealed};

pub struct Square;

// Names and visibilities are written as strings across these macros, so the
// bare identifier is refused rather than accepted as a second spelling.
#[enumerate(name = Shapes)]
#[sealed(Square)]
pub trait Shape {}

impl Shape for Square {}

pub struct Circle;

#[enumerate(match_any(walk))]
#[sealed(Circle)]
pub trait Round {}

impl Round for Circle {}

fn main() {}
