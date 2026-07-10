[![Rust CI](https://github.com/mozilla/thin-vec/actions/workflows/rust.yml/badge.svg)](https://github.com/mozilla/thin-vec/actions) [![crates.io](https://img.shields.io/crates/v/thin-vec.svg)](https://crates.io/crates/thin-vec) [![Docs](https://docs.rs/thin-vec/badge.svg)](https://docs.rs/thin-vec)

# thin-vec

ThinVec is a Vec that stores its length and capacity inline, making it take up
less space.

This fork focuses exclusively on a compact, high-performance native Rust vector.

See [`benches/README.md`](benches/README.md) for CPU and allocation benchmarks
that compare `ThinVec` with `Vec`.
