//! Compile-failure fixtures.
//!
//! The `.stderr` files capture rustc's exact diagnostic wording, which changes
//! between releases, so these are pinned to one toolchain rather than run on
//! whatever compiler happens to be installed. CI runs them in a single job that
//! names that version; see `.github/workflows/ci.yml`.
//!
//! To run them locally, on the pinned toolchain:
//!
//! ```text
//! TRYBUILD_UI=1 cargo test -p closed-trait --test ui
//! ```
//!
//! To regenerate every fixture after changing a diagnostic:
//!
//! ```text
//! TRYBUILD=overwrite TRYBUILD_UI=1 cargo test -p closed-trait --test ui
//! ```

#[test]
fn ui() {
    if std::env::var_os("TRYBUILD_UI").is_none() {
        eprintln!(
            "skipping UI tests: they are pinned to one rustc version. \
             Set TRYBUILD_UI=1 to run them."
        );
        return;
    }

    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
