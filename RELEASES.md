# JackVec 0.1.0 (unreleased)

* Establish the `jack-vec` package, `jack_vec` crate path, `JackVec` type, and
  `jack_vec!` macro while
  preserving explicit attribution to Mozilla's ThinVec.
* Focus the implementation on native Rust and remove Gecko/nsTArray FFI and
  AutoThinVec compatibility.
* Reduce the allocation header from 16 to 8 bytes on 64-bit targets, with a
  maximum capacity of `u32::MAX` elements.
* Optimize push, append, extend, resize, retain, deduplication, array/macro
  construction, and ownership conversions.
* Strengthen panic safety for cloning, extension, resizing, retain, and dedup.
* Add reproducible CPU, allocation, code-size, and cross-platform benchmark
  tooling.
* Set the minimum supported Rust version to 1.86.

# ThinVec upstream history

The entries below predate the JackVec fork and are retained for provenance.

# Versions 0.2.17 and 0.2.18 (2026-04-29)
* Fix compiling some feature combinations in no_std mode

# Version 0.2.16 (2026-04-14)
* Fix reserve() on auto arrays in gecko-ffi mode.
* Fix two double-drop issues with ThinVec::clear() and ThinVec::into_iter()
  when the Drop implementation of the item panics.

# Version 0.2.15 (2026-04-08)
* Support AutoTArrays created from Rust in Gecko FFI mode.
* Add extract_if.
* Add const new() support behind feature flag.
* Fix `thin_vec` macro not being hygienic when recursing
* Improve extend() performance.

# Version 0.2.14 (2025-03-23)
* Add "malloc_size_of" feature for heap size measurement support

# Version 0.2.13 (2023-12-02)

* add default-on "std" feature for no_std support
* added has_capacity method for checking if something is the empty singleton
* marked more things as `#[inline]`
* added license files
* appeased Clippy

# Previous Versions

*shrug*
