use closed_trait::sealed;

#[sealed]
pub trait Foo {}

pub struct Nope;

impl Foo for Nope {}

fn main() {}
