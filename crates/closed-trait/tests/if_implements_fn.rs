//! `#[if_implements_fn]`: the types the generated macro cannot infer.
//!
//! A parameter is settled either by an argument whose type names it, or by what
//! is done with the result, since the closure's return type stays open. One the
//! signature never names is settled by neither, and is given after a `;`.

use closed_trait::if_implements_fn;
use std::fmt::{Debug, Display};

/// `T` is named nowhere in the signature, so it has to be given.
#[if_implements_fn]
fn free<T: Default + Debug>(value: impl Display, times: usize) -> String {
    format!("{value} {times} {:?}", T::default())
}

/// `T` has to be given, `U` is settled by its argument: the list carries both.
#[if_implements_fn]
fn mixed<T: Default + Debug, U: Display>(value: impl Display, other: U) -> String {
    format!("{value} {other} {:?}", T::default())
}

/// The subject is a named parameter rather than an `impl Bound`, so `free`'s
/// declaration order has to be respected while skipping it.
#[if_implements_fn]
fn named<S: Display, T: Default + Debug>(value: S) -> String {
    format!("{value} {:?}", T::default())
}

/// Settled by the argument, so the `;` is accepted but never needed.
#[if_implements_fn]
fn shown<U: Display>(value: impl Display, other: U) -> String {
    format!("{value}/{other}")
}

/// Settled by what is done with the result rather than by any argument.
#[if_implements_fn]
fn make<T: Default>(value: impl Display, times: usize) -> T {
    let _ = (value, times);
    T::default()
}

#[test]
fn a_type_the_signature_never_names_is_given_after_a_semicolon() {
    let call = try_free!(6; String).expect("`u8` is `Display`");
    assert_eq!(call(6, 1), "6 1 \"\"");
}

#[test]
fn the_result_is_still_an_fn() {
    let call = try_free!(6; String).expect("`u8` is `Display`");
    assert_eq!(call(6, 1), "6 1 \"\"");
    assert_eq!(call(7, 2), "7 2 \"\"");
}

#[test]
fn a_trailing_comma_is_accepted() {
    let call = try_free!(6; String,).expect("`u8` is `Display`");
    assert_eq!(call(6, 1), "6 1 \"\"");
}

#[test]
fn underscore_stands_for_one_inference_settles() {
    let call = try_mixed!(6; String, _).expect("`u8` is `Display`");
    assert_eq!(call(6, 'x'), "6 x \"\"");
}

#[test]
fn the_subjects_own_parameter_is_not_one_of_them() {
    let call = try_named!(6; u8).expect("`u8` is `Display`");
    assert_eq!(call(6), "6 0");
}

#[test]
fn the_types_are_optional_where_the_arguments_settle_them() {
    let call = try_shown!(6).expect("`u8` is `Display`");
    assert_eq!(call(6, 'y'), "6/y");

    let call = try_shown!(6; char).expect("`u8` is `Display`");
    assert_eq!(call(6, 'z'), "6/z");
}

#[test]
fn the_call_site_settles_a_type_only_the_result_names() {
    let call = try_make!(6).expect("`u8` is `Display`");
    let made: String = call(6, 1);
    assert_eq!(made, "");
}

#[test]
fn the_list_is_accepted_even_where_there_is_nothing_to_fix() {
    // Not a rule that refuses to match: every list is written out as a turbofish,
    // and rustc reports on the ones that do not fit. An empty one always fits.
    #[if_implements_fn]
    fn plain(value: impl Display) -> String {
        format!("{value}")
    }

    assert_eq!(try_plain!(6).expect("`u8` is `Display`")(6), "6");
    assert_eq!(try_plain!(6;).expect("`u8` is `Display`")(6), "6");
}

#[test]
fn a_const_argument_can_be_given() {
    // A `ty` fragment matches no const argument, so the list is taken as tokens
    // and handed to the turbofish as written.
    #[if_implements_fn]
    fn sized<const N: usize>(value: impl Display) -> String {
        format!("{value} {N}")
    }

    assert_eq!(try_sized!(6; 3).expect("`u8` is `Display`")(6), "6 3");
    assert_eq!(
        try_sized!(6; { 2 + 1 }).expect("`u8` is `Display`")(6),
        "6 3"
    );
}

#[test]
fn a_const_fn_keeps_its_constness() {
    // Emitted as written, so a direct call is still const. Only the macro's path is
    // not, which costs nothing: what it hands back is called at run time anyway.
    #[if_implements_fn]
    const fn doubled(value: impl Copy, extra: u32) -> u32 {
        let _ = value;
        extra * 2
    }

    const TWELVE: u32 = doubled(1u8, 6);
    assert_eq!(TWELVE, 12);
    assert_eq!(try_doubled!(6u8).expect("`u8` is `Copy`")(6u8, 21), 42);
}

#[test]
fn a_semicolon_may_carry_nothing() {
    assert_eq!(try_shown!(6;).expect("`u8` is `Display`")(6, 'y'), "6/y");
}

#[test]
fn the_subjects_parameter_avoids_a_name_the_function_declares() {
    // The annotation names no parameter, so the probe declares one. Calling it
    // `T` here would collide with the function's own, which `get` declares.
    #[if_implements_fn]
    fn collides<T: Default + Debug>(value: impl Display) -> String {
        format!("{value} {:?}", T::default())
    }

    assert_eq!(try_collides!(6; u8).expect("`u8` is `Display`")(6), "6 0");
}

mod visibility {
    use super::*;

    /// `pub`, but the macro is capped at the crate: it is not exported.
    #[if_implements_fn]
    pub fn capped(value: impl Display) -> String {
        format!("{value}")
    }

    /// Narrowed further, so the macro stays in this module alone.
    #[if_implements_fn(vis = "pub(self)")]
    pub fn hidden(value: impl Display) -> String {
        format!("{value}")
    }

    pub fn from_here() -> String {
        try_hidden!(6).expect("`u8` is `Display`")(6)
    }
}

/// The pattern `#[macro_export]` could not support: one name in the crate root
/// per function means these two would collide there.
mod json {
    use super::*;

    #[if_implements_fn]
    pub fn parse(value: impl Display) -> String {
        format!("j{value}")
    }
}

mod yaml {
    use super::*;

    #[if_implements_fn]
    pub fn parse(value: impl Display) -> String {
        format!("y{value}")
    }
}

#[test]
fn the_macro_is_capped_at_the_crate() {
    use visibility::{capped, try_capped};
    assert_eq!(try_capped!(6).expect("`u8` is `Display`")(6), "6");
    assert_eq!(visibility::from_here(), "6");
}

#[test]
fn two_functions_may_share_a_name_in_different_modules() {
    // A scope each, since the expansion resolves `parse` where it is written, so
    // both names have to be in scope and only one `parse` can be.
    fn from_json() -> String {
        use json::{parse, try_parse};
        try_parse!(6).expect("`u8` is `Display`")(6)
    }
    fn from_yaml() -> String {
        use yaml::{parse, try_parse};
        try_parse!(6).expect("`u8` is `Display`")(6)
    }

    assert_eq!(from_json(), "j6");
    assert_eq!(from_yaml(), "y6");
}

#[test]
fn a_pub_fn_in_a_body_needs_no_option() {
    // Nothing is planted in the crate root, so `pub` here is as inert as rustc
    // treats it: no export naming a function that cannot be reached.
    #[if_implements_fn]
    pub fn nested(value: impl Display, size: i32) -> String {
        format!("{value} {size}")
    }

    assert_eq!(try_nested!(6).expect("`u8` is `Display`")(6, 4), "6 4");
}

#[test]
fn the_bounds_are_still_what_decides() {
    struct Opaque;
    assert!(try_free!(Opaque; String).is_none());
    assert!(try_mixed!(Opaque; String, char).is_none());
    assert!(try_named!(Opaque; u8).is_none());
}

/// A minimal executor, so the async case needs no dependency.
fn block_on<F: core::future::Future>(mut future: F) -> F::Output {
    use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    fn nothing(_: *const ()) {}
    fn clone(pointer: *const ()) -> RawWaker {
        RawWaker::new(pointer, &VTABLE)
    }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, nothing, nothing, nothing);

    let waker = unsafe { Waker::from_raw(RawWaker::new(core::ptr::null(), &VTABLE)) };
    let mut context = Context::from_waker(&waker);
    let mut future = unsafe { core::pin::Pin::new_unchecked(&mut future) };
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

#[test]
fn an_async_fn_hands_back_a_future() {
    // Nothing is awaited in the expansion: what comes back yields the function's
    // future, and the caller decides where to await it.
    #[if_implements_fn]
    async fn slow(value: impl Display, extra: usize) -> String {
        format!("{value}{extra}")
    }

    let call = try_slow!(6).expect("`u8` is `Display`");
    assert_eq!(block_on(call(6, 1)), "61");
    // Still an `Fn`, so a second future can be made from it.
    assert_eq!(block_on(call(7, 2)), "72");

    struct Opaque;
    assert!(try_slow!(Opaque).is_none());
}

#[test]
fn the_first_parameter_may_be_taken_by_reference() {
    #[if_implements_fn]
    fn shared(value: &impl Display, tag: &str) -> String {
        format!("{tag}={value}")
    }

    #[if_implements_fn]
    fn exclusive(value: &mut impl Display, tag: &str) -> String {
        format!("{tag}={value}")
    }

    let mut n = 6u8;
    assert_eq!(try_shared!(&n).expect("`u8` is `Display`")(&n, "n"), "n=6");
    assert_eq!(
        try_exclusive!(&mut n).expect("`u8` is `Display`")(&mut 6u8, "n"),
        "n=6"
    );
}

#[test]
fn bounds_may_be_written_in_a_where_clause() {
    #[if_implements_fn]
    fn tagged<T>(value: T, tag: &str) -> String
    where
        T: Display + Clone,
    {
        format!("{tag}={}", value.clone())
    }

    assert_eq!(try_tagged!(6).expect("`u8` is both")(6, "n"), "n=6");

    struct Opaque;
    assert!(try_tagged!(Opaque).is_none());
}

#[test]
fn the_function_may_call_itself() {
    // The function is emitted as written rather than copied into the macro, so a
    // recursive body resolves against the real item.
    #[if_implements_fn]
    fn countdown(value: impl Display + Clone, n: usize) -> String {
        match n {
            0 => format!("{value}"),
            _ => countdown(value, n - 1),
        }
    }

    assert_eq!(try_countdown!(6).expect("`u8` is `Display`")(6, 3), "6");
}

#[test]
fn a_parameter_may_be_a_pattern_rather_than_a_name() {
    // Nothing to forward it by, so the expansion names it itself.
    #[if_implements_fn]
    fn pair(value: impl Display, (a, b): (u8, u8)) -> String {
        format!("{value}{a}{b}")
    }

    assert_eq!(try_pair!(6).expect("`u8` is `Display`")(6, (1, 2)), "612");
}

#[test]
fn several_bounds_behind_a_reference_need_parentheses() {
    #[if_implements_fn]
    fn cloned(value: &(impl Display + Clone), tag: &str) -> String {
        format!("{tag}={}", value.clone())
    }

    assert_eq!(try_cloned!(&6u8).expect("`u8` is both")(&6, "n"), "n=6");

    struct Opaque;
    assert!(try_cloned!(&Opaque).is_none());
}

#[test]
fn inline_bounds_and_a_where_clause_are_taken_together() {
    // Both halves have to reach the probe: with only one of them, either bound
    // would go untested and the wrong branch would answer.
    // Split on purpose: clippy would rather they were written in one place, but
    // both halves reaching the probe is what this checks.
    #[allow(clippy::multiple_bound_locations)]
    #[if_implements_fn]
    fn both<T: Display>(value: T, tag: &str) -> String
    where
        T: Clone,
    {
        format!("{tag}={}", value.clone())
    }

    assert_eq!(try_both!(6).expect("`u8` is both")(6, "n"), "n=6");

    #[derive(Clone)]
    struct OnlyClone;
    assert!(try_both!(OnlyClone).is_none());

    struct OnlyDisplay;
    impl Display for OnlyDisplay {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("d")
        }
    }
    assert!(try_both!(OnlyDisplay).is_none());
}

#[test]
fn a_lifetime_the_function_declares_is_carried() {
    // Named on purpose, elidable though it is: a declared lifetime is what the
    // probe's constructor has to carry.
    #[allow(clippy::needless_lifetimes)]
    #[if_implements_fn]
    fn borrowed<'a>(value: &'a impl Display, tag: &str) -> String {
        format!("{tag}={value}")
    }

    let n = 6u8;
    assert_eq!(
        try_borrowed!(&n).expect("`u8` is `Display`")(&n, "n"),
        "n=6"
    );
}

#[test]
fn the_result_may_be_an_opaque_type() {
    // What comes back names the return type, so an `impl Trait` return would
    // capture the probe's lifetime if the probe were borrowed rather than copied.
    #[if_implements_fn]
    fn wrapped(value: impl Display) -> impl Display {
        format!("<{value}>")
    }

    let call = try_wrapped!(6).expect("`u8` is `Display`");
    assert_eq!(call(6).to_string(), "<6>");
    assert_eq!(call(7).to_string(), "<7>");
}

#[test]
fn the_result_may_name_the_tested_type() {
    #[if_implements_fn]
    fn twice<T: Display + Clone>(value: T, _tag: &str) -> (T, T) {
        (value.clone(), value)
    }

    let (a, b) = try_twice!(6u8).expect("`u8` is both")(6, "n");
    assert_eq!((a, b), (6, 6));
}

#[test]
fn the_macro_may_be_given_a_name() {
    #[if_implements_fn(name = "probe_display")]
    fn renamed(value: impl Display) -> String {
        format!("{value}")
    }

    assert_eq!(probe_display!(6).expect("`u8` is `Display`")(6), "6");
}

mod named {
    use super::*;

    #[if_implements_fn(vis = "pub(crate)", name = "probe_it")]
    pub fn shown_here(value: impl Display) -> String {
        format!("{value}")
    }
}

#[test]
fn a_name_and_a_visibility_may_be_given_together() {
    use named::{probe_it, shown_here};
    assert_eq!(probe_it!(6).expect("`u8` is `Display`")(6), "6");
    assert_eq!(shown_here(6), "6");
}
