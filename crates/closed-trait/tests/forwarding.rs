//! Forwarding the trait's methods onto the enum, written by hand with the match
//! macro. Every case that made a generated version awkward — returning `Self`,
//! `unsafe`, `async`, a `where Self: ..` bound — is here an ordinary decision
//! taken in the impl, where the reader can see it.

use core::future::Future;
use core::pin::pin;
use core::task::{Context, Poll, Waker};

use closed_trait::{enumerate, sealed};

#[derive(Clone)]
pub struct Square {
    pub side: i32,
}
#[derive(Clone)]
pub struct Circle {
    pub radius: i32,
}

#[allow(async_fn_in_trait)]
#[enumerate(match_any)]
#[sealed(Square, Circle)]
pub trait Shape {
    fn area(&self) -> i32;

    fn doubled(self) -> Self
    where
        Self: Sized;

    async fn area_later(&self) -> i32;

    /// # Safety
    ///
    /// Nothing here is actually unsafe; the qualifier is what is under test.
    unsafe fn area_unchecked(&self) -> i32;

    fn duplicate(&self) -> Self
    where
        Self: Clone;
}

impl Shape for Square {
    fn area(&self) -> i32 {
        self.side * self.side
    }
    fn doubled(self) -> Self {
        Square {
            side: self.side * 2,
        }
    }
    async fn area_later(&self) -> i32 {
        self.area()
    }
    unsafe fn area_unchecked(&self) -> i32 {
        self.area()
    }
    fn duplicate(&self) -> Self {
        self.clone()
    }
}

impl Shape for Circle {
    fn area(&self) -> i32 {
        3 * self.radius * self.radius
    }
    fn doubled(self) -> Self {
        Circle {
            radius: self.radius * 2,
        }
    }
    async fn area_later(&self) -> i32 {
        self.area()
    }
    unsafe fn area_unchecked(&self) -> i32 {
        self.area()
    }
    fn duplicate(&self) -> Self {
        self.clone()
    }
}

impl AnyShape {
    pub fn area(&self) -> i32 {
        match_any_shape!(self, s => s.area())
    }

    /// The trait's `Self` is the variant; what it should mean here is a choice,
    /// and this one puts the result back into the enum.
    pub fn doubled(self) -> Self {
        match_any_shape!(self, s => AnyShape::from(s.doubled()))
    }

    pub async fn area_later(&self) -> i32 {
        match_any_shape!(self, s => s.area_later().await)
    }

    /// # Safety
    ///
    /// Inherited from the method it dispatches to, which is to say: none.
    pub unsafe fn area_unchecked(&self) -> i32 {
        match_any_shape!(self, s => unsafe { s.area_unchecked() })
    }

    /// `where Self: Clone` on the trait constrains the variant, and here that
    /// is simply true of both, so nothing needs saying.
    pub fn duplicate(&self) -> Self {
        match_any_shape!(self, s => AnyShape::from(s.duplicate()))
    }
}

/// Enough of an executor for futures ready on the first poll, which these are.
fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("the forwarded future yielded, which none of these do"),
    }
}

#[test]
fn plain_and_self_returning() {
    let shape = AnyShape::from(Square { side: 3 });
    assert_eq!(shape.area(), 9);

    match AnyShape::from(Square { side: 3 }).doubled() {
        AnyShape::Square(square) => assert_eq!(square.side, 6),
        AnyShape::Circle(_) => unreachable!(),
    }
}

#[test]
fn async_and_unsafe() {
    let shape = AnyShape::from(Circle { radius: 2 });
    assert_eq!(block_on(shape.area_later()), 12);
    assert_eq!(unsafe { shape.area_unchecked() }, 12);
}

#[test]
fn a_self_bounded_method() {
    let shape = AnyShape::from(Square { side: 4 });
    assert_eq!(shape.duplicate().area(), 16);
}
