# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-09-10

### Added

- `#[if_implements_fn]`, which generates a macro instantiating the annotated
  function for a type when that type satisfies the bounds on the function's
  first parameter. `describe` gets `try_describe!`, which returns `Some` of an
  `Fn` for that instantiation or `None`. Takes `vis = ".."` and `name = ".."`,
  and type or const arguments after a `;`.
- `#[enumerate]` accepts entries with a `for<..>` binder. The instantiation says
  which of the trait's parameters each bound name stands for, so `for<U>
  Boxed<U>: Store<U>` reaches the enum as the variant `Boxed<T>`, and the
  binder's bounds are declared on the enum.
- `#[sealed]` accepts an empty list, which seals the trait against everything.

### Changed

- **Breaking.** A bare name in a `#[sealed(..)]` entry no longer resolves
  against the trait's parameters. It is whatever is in scope where the attribute
  is written, and only the entry's own `for<..>` declares a parameter.
- **Breaking.** `#[enumerate]`'s option values that name an item are written as
  strings: `name = "Shapes"` and `match_any("walk")`.
- **Breaking.** A parameter a `for<..>` declares has to be used by the entry,
  in its type or in its instantiation. An unused type or const was rustc's
  `unconstrained parameter` error spanned on the attribute; an unused lifetime
  was accepted and meant nothing.
- **Breaking.** An option, a group, or an attribute written twice is an error.
  Previously `match_any` took the last, `no_bridge` and `skip` did nothing,
  two `attrs` were merged onto the enum, and a repeated `#[sealed(..)]` or
  `#[enumerate]` produced duplicate items.
- `match_any(..)` accepts a trailing comma, as every other list does, and
  refuses a second name with a message rather than `unexpected token`.

### Fixed

- `#[if_implements_fn(vis = "..")]` no longer accepts a visibility wider than
  the function's. The macro expands to a call, so a wider one failed at the call
  site inside an expansion the caller never wrote.
- `#[sealed]` declares the trait parameters an entry's instantiation names, and
  gives its generated assertion a parameter name the trait has not spent.

## [0.1.2] - 2026-08-27

- Documentation corrections.

## [0.1.1] - 2026-08-27

- Explained why forwarding impls are not generated, and tightened the README.
- Fixed the license links in the README.
- Pinned the UI suite's toolchain and installed `rust-src` for it, so stdlib
  snippets in the expected output match.

## [0.1.0] - 2026-08-27

First release: `#[sealed]` and `#[enumerate]`.
