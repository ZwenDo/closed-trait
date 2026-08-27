# closed-trait-macros

The attribute macros behind [`closed-trait`](https://crates.io/crates/closed-trait). This crate is an implementation
detail — it exports `#[sealed]` and `#[enumerate]` but not the traits their
expansions refer to, so on its own it does not compile into anything useful.

Depend on the facade instead, which re-exports both attributes:

```toml
[dependencies]
closed-trait = "<latest-version>"
```

Everything — what the macros do, their options, and the traits they generate
impls for — is documented there: <https://docs.rs/closed-trait>.

## License

[MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

