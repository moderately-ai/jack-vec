[![Rust CI](https://github.com/mozilla/thin-vec/actions/workflows/rust.yml/badge.svg)](https://github.com/mozilla/thin-vec/actions) [![crates.io](https://img.shields.io/crates/v/thin-vec.svg)](https://crates.io/crates/thin-vec) [![Docs](https://docs.rs/thin-vec/badge.svg)](https://docs.rs/thin-vec)

# JackVec

> Jack be nimble. Jack be quick.

JackVec is the working name for this native, performance-focused fork of
Mozilla's [`thin-vec`](https://github.com/mozilla/thin-vec) crate. ThinVec's
one-word vector design stores its length and capacity in the allocation rather
than in the collection value, reducing the footprint of empty-heavy and deeply
nested data structures.

This project builds directly on the design and implementation work of ThinVec's
original authors and Mozilla contributors. The existing Rust package, type, and
macro remain `thin-vec`, `ThinVec`, and `thin_vec!` while the JackVec rename is
prepared as a separate, deliberate API transition.

See [`benches/README.md`](benches/README.md) for CPU and allocation benchmarks
that compare `ThinVec` with `Vec`.
