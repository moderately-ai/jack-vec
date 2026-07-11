[![Rust CI](https://github.com/moderately-ai/jack-vec/actions/workflows/rust.yml/badge.svg)](https://github.com/moderately-ai/jack-vec/actions) [![crates.io](https://img.shields.io/crates/v/jack-vec.svg)](https://crates.io/crates/jack-vec) [![Docs](https://docs.rs/jack-vec/badge.svg)](https://docs.rs/jack-vec)

# JackVec

> Jack be nimble. Jack be quick.

JackVec is a native, performance-focused descendant of
Mozilla's [`thin-vec`](https://github.com/mozilla/thin-vec) crate. ThinVec's
one-word vector design stores its length and capacity in the allocation rather
than in the collection value, reducing the footprint of empty-heavy and deeply
nested data structures.

This project builds directly on the design and implementation work of ThinVec's
original authors and Mozilla contributors. Its Cargo package, Rust crate path,
primary type, and construction macro are `jack-vec`, `jack_vec`, `JackVec`, and
`jack_vec!`.

JackVec retains the one-word owner and shared allocation-free empty singleton,
uses an eight-byte allocation header on 64-bit targets, and limits capacity to
`u32::MAX` elements. It targets native Rust rather than Gecko/nsTArray FFI.

```toml
[dependencies]
jack-vec = "0.1"
```

```rust
use jack_vec::{jack_vec, JackVec};

let values: JackVec<_> = jack_vec![1, 2, 3];
assert_eq!(values.as_slice(), &[1, 2, 3]);
```

See [`benches/README.md`](benches/README.md) for CPU and allocation benchmarks
that compare `JackVec` with `Vec`.

## Lineage and attribution

JackVec is derived from ThinVec and retains its MIT OR Apache-2.0 licensing.
Copyright and authorship credit for the original design and implementation
belongs to ThinVec's authors and Mozilla contributors; JackVec's history and
release notes preserve that provenance explicitly.
