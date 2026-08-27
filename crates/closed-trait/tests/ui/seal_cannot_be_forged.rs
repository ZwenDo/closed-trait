use closed_trait::sealed;

pub struct Square;
pub struct Sneaky;

#[sealed(Square)]
pub trait Shape {}

impl Shape for Square {}

// Naming the marker is not enough: its own supertrait is private one level down.
impl __sealed_Shape::Sealed for Sneaky {}

fn main() {}
