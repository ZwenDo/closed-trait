use closed_trait::sealed;

pub struct Square;
pub struct Circle;

// A trait is sealed to one list, so two attributes are one too many. Both are
// dropped rather than one expanded: sealing from either would refuse every type
// the other listed.
#[sealed(Square)]
#[sealed(Circle)]
pub trait Shape {}

impl Shape for Square {}
impl Shape for Circle {}

fn main() {}
