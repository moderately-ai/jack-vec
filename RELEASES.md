# Unreleased
* Rename the fork to JackVec: the package is `jackvec`, the primary type is
  `JackVec`, and the construction macro is `jack_vec!`. The project remains
  directly derived from and attributed to Mozilla's ThinVec.
* Raise the minimum supported Rust version to 1.86.
* Add CPU and allocation benchmarks comparing ThinVec with Vec.

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
