[![Rust CI](https://github.com/tomsanbear/thin-vec/actions/workflows/rust.yml/badge.svg)](https://github.com/tomsanbear/thin-vec/actions) [![crates.io](https://img.shields.io/crates/v/jackvec.svg)](https://crates.io/crates/jackvec) [![Docs](https://docs.rs/jackvec/badge.svg)](https://docs.rs/jackvec)

# JackVec

> Jack be nimble. Jack be quick.

JackVec is the working name for this native, performance-focused fork of
Mozilla's [`thin-vec`](https://github.com/mozilla/thin-vec) crate. ThinVec's
one-word vector design stores its length and capacity in the allocation rather
than in the collection value, reducing the footprint of empty-heavy and deeply
nested data structures.

This project builds directly on the design and implementation work of ThinVec's
original authors and Mozilla contributors. Its Rust package, primary type, and
construction macro are `jackvec`, `JackVec`, and `jack_vec!`.

See [`benches/README.md`](benches/README.md) for CPU and allocation benchmarks
that compare `JackVec` with `Vec`.
