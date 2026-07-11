# JackVec performance roadmap

This document is the active project ledger for performance work on the
`tomsanbear/thin-vec` fork under the JackVec project name.
JackVec is a direct descendant of Mozilla's ThinVec; historical ThinVec names in
this ledger are retained so results and commit records remain accurate. Update
the ledger whenever an experiment starts, finishes, changes priority, or produces
a reusable finding. Record rejected ideas and the evidence behind them so they
are not repeatedly rediscovered.

## Objective

Push the one-word vector design toward state-of-the-art CPU and memory behavior,
with deeply nested, empty-heavy owned ASTs as the proving workload.

The central hypothesis is:

> ThinVec chose the right one-word owner. JackVec can improve its allocation
> representation and construction algorithms while preserving that final density.

## Non-negotiable invariants

- The final collection occupies exactly one machine word.
- `Option<Collection<T>>` remains one word through the non-null pointer niche.
- Empty collections share a singleton and do not allocate.
- Elements remain contiguous and available as an ordinary slice.
- Allocation and deallocation use reconstructable, matching layouts.
- `len` always describes the initialized prefix except inside explicitly guarded
  drain/extract/splice states.
- Panic paths never double-drop; partial initialization is guarded.
- Leaking an iterator may leak values but must remain memory-safe.
- `Send` and `Sync` behavior remains determined by `T`.
- Strict-provenance Miri validation is required for pointer tagging or alternate
  representations.
- Final inline element storage is out of scope because it destroys empty-container
  density in recursive structures.

## Repository state

- Fork: `https://github.com/tomsanbear/thin-vec`
- Canonical branch: `jackvec`
- Working branch: `benchmarks/allocator-usable-size`
- Initial benchmark commit: `5e4845a`
- Refined timing-boundary commit: `f8fa1e8`
- Persistent benchmark checkout: `catalyzed-builder:~/thin-vec`
- Benchmark toolchain: Rust 1.86
- Benchmark CPU: Ryzen 9 7950X3D, pinned to CPU 0 on the 96 MiB L3 CCD
- Benchmark OS: Ubuntu, Linux 5.15, glibc 2.35
- Allocator warning: the login environment preloads tcmalloc globally. Every CPU
  result must explicitly state whether preload was inherited or cleared; “glibc
  System” means the runner recorded an empty effective preload environment.

## Experiment record

### Nested metadata scan (`benchmarks/metadata-scan`)

- Status: pre-registered; implementation pending
- Baseline commit: `48ddf42`
- Question: does JackVec's allocation-header metadata access create enough cost in
  an empty-heavy recursive workload to justify investigating a cached/tagged
  length, or does its one-word outer representation already offset the indirection?
- Workload: build the existing deterministic sparse population of 10,000 nested
  `u64` containers before timing, then scan only `len`, `is_empty`, and `capacity`
  into a checksum. Compare `Vec` and JackVec in one Criterion group. Do not touch
  elements, allocate, mutate, or destroy containers inside the timed operation.
- Rationale for one workload: sparse includes 80% shared empty singletons, 15%
  singleton allocations, and 5% four-element allocations. It exercises both the
  compact outer array and scattered nonempty headers without adding redundant
  empty/small benchmark variants.
- Interpretation: this is a falsification benchmark, not an optimization claim.
  If JackVec is competitive or faster, reject pointer-tagged cached length for lack
  of demonstrated headroom. If materially slower, inspect optimized code and cache
  behavior before considering a prototype; the benchmark alone cannot distinguish
  pointer chasing from layout, cache, or compiler effects.
- Measurement: seven alternating paired rounds on pinned Linux with preload
  cleared, plus a macOS comparison. Retain raw estimates and inspect focused
  assembly. Require a repeatable JackVec regression above 5% before a pointer-tag
  prototype becomes eligible; smaller differences do not justify its unsafe and
  semantic complexity.
- Scope: one sparse metadata benchmark and its helper only. No library, layout,
  tagging, API, allocation, or growth-policy changes.

### Allocator usable-size and reallocation diagnostics (`benchmarks/allocator-usable-size`)

- Status: accepted as diagnostic tooling; no library policy change justified
- Baseline commit: `fe7aa89`
- Question: where does JackVec's requested allocation size map to a smaller physical
  allocator class, and when does growth remain in place versus move/copy? Requested
  bytes alone cannot answer either question.
- Instrumentation: extend only the deterministic allocation runner with live/peak/
  post-drop usable bytes and moved/in-place reallocation counts. Use macOS
  `malloc_size` and glibc Linux `malloc_usable_size`; preserve requested-byte
  tracking and clearly label unsupported platforms rather than treating requested
  bytes as measured usable bytes.
- Measurement matrix: existing empty/sparse/four-element nested workloads and
  growing/reserved lengths 1, 4, and 1,024 under macOS System malloc and cleared-
  preload glibc System on the builder. Record allocator and OS independently.
- Acceptance: instrumentation must observe zero live requested and usable bytes
  after every workload, never report usable bytes below requested bytes, preserve
  existing allocation/reallocation/deallocation counts, and avoid all CPU benchmark
  paths. Run all-target, Rust 1.86, Clippy, and allocation smoke gates on both hosts.
- Decision use: only propose allocator-class or growth-policy changes where these
  measurements expose a concrete size-class boundary, retained-waste pattern, or
  repeatable moved-reallocation cost. This experiment makes no speedup claim.
- Scope: benchmark allocator wrapper and documentation only. No library source,
  growth factor, capacity, layout, or public API changes.
- Implementation commit: `8174aca`
- Instrumentation result: all workloads return both requested and usable live
  bytes to zero, usable bytes never fall below requested bytes, and moved plus
  in-place reallocations equal total reallocations. Allocation lifecycle counts
  are unchanged. The runner labels macOS `malloc_size`, Linux
  `malloc_usable_size`, and the requested-byte fallback explicitly.
- macOS result: allocator classes erase much of the requested-byte advantage for
  dense four-element nesting: JackVec requests 480,000 versus Vec's 560,000 bytes,
  but uses 578,304 versus 582,144 bytes physically, a 3,840-byte or 0.66% usable
  advantage. Sparse nesting remains a strong physical win at 194,304 versus
  326,144 bytes (40.42% less). At 1,024 `u64` elements, Vec's 8,192-byte request
  remains 8,192 usable bytes while JackVec's 8,200-byte combined allocation maps
  to 8,704; a 1,032-byte `JackVec<u8>` likewise maps to 1,536. These are genuine
  Darwin allocator-class cliffs, not extra bytes requested by JackVec.
- Linux glibc result: the same boundaries behave differently. Both Vec's 8,192-
  byte storage and JackVec's 8,200-byte combined allocation report 8,200 usable
  bytes; `JackVec<u8>` at 1,032 requested also reports exactly 1,032 usable.
  Four-element nested JackVec uses 480,024 versus Vec's 640,008 bytes (25.00%
  less), and sparse nesting uses 160,008 versus 320,008 (50.00% less). The compact
  header therefore produces substantially larger physical wins under this glibc
  workload than under macOS malloc.
- Reallocation result: growing both implementations to 1,024 elements performs
  eight reallocations. The observed moved/in-place split varied by allocator and
  even between macOS process runs, so it is retained as a deterministic-workload
  diagnostic and must not be interpreted as a CPU result or stable allocator
  guarantee.
- Decision: retain the diagnostic columns. Reject a generalized allocator-class-
  aware growth change: the motivating 8,200-byte cliff exists on macOS but not on
  the measured glibc host, allocator usable size is not part of Rust's allocator
  contract, and changing logical capacity to consume undocumented slack would
  require requesting/deallocating a matching rounded layout. Any future attempt
  must be explicitly allocator-specific, opt-in, and benchmarked as a separate
  policy rather than changing JackVec's default semantics.
- Validation: macOS and Linux allocation runs, 103 all-target tests on both hosts,
  formatting, diff checks, local warning-denied Clippy, Rust 1.86 checks, and a
  Linux Rust 1.86 allocation-bench Clippy lane pass. Rust 1.86's full-library
  Clippy additionally reports six pre-existing `Drain` needless-lifetime lints;
  these are outside this benchmark-only branch and were allowed only to isolate
  the changed target.
- Artifact: `catalyzed-builder:~/thin-vec/benchmark-results/jackvec-allocator-usable-linux-20260711/run.log`.

### Exact macro construction (`perf/exact-macro-construction`)

- Status: accepted; temporary benchmark removed
- Baseline implementation commit: `3e25d5e`
- Hypothesis: `jack_vec![a, b, ...]` knows its exact arity at compile time but still
  allocates and calls the general push path for every expression. Routing nonempty
  literal-form macros through the accepted direct `From<[T; N]>` relocation should
  allocate once, move the initialized array once, and publish length once while
  preserving left-to-right expression evaluation and panic cleanup.
- Primary workload: construct four `u64` values with `jack_vec!`; require at least
  10% improvement in seven paired pinned-Linux rounds under the standard cleared
  allocator protocol. Secondary singleton construction must not regress beyond 1%.
  Setup is only scalar expression production; allocation, initialization, length
  publication, and returned-vector destruction remain consistently bounded by the
  Criterion batch routine.
- Memory gate: exact requested bytes, one allocation, zero reallocations, one
  deallocation, capacity equal to arity, and zero live bytes after drop must remain
  unchanged.
- Semantic gates: zero arguments still use the singleton; one, multiple, trailing
  comma, non-Copy owning values, left-to-right evaluation, and panic during a later
  expression must drop each earlier value exactly once. Preserve macro hygiene and
  `$crate` behavior from an external call site.
- Code-size gate: reuse the existing array relocation monomorphization; reject an
  unexplained complete `.text` increase even if timing improves. Run all feature,
  MSRV, docs, Clippy, and strict-provenance Miri gates.
- Scope: literal-list macro arm, focused temporary 1/4 benchmark, and semantic
  tests only. Do not add public one/two constructors, change repeat syntax, or mix
  `from_fn`, growth policy, or inline scratch.
- Baseline harness commit: `33fc702`
- Candidate commit: `ae3856f`
- CPU result: four-element construction improved 11.50%, from 11.68 ns to
  10.34 ns, with every round favorable and interval -14.21%..-10.37%, clearing
  the 10% primary threshold. Singleton construction also improved 9.08%, from
  10.65 ns to 9.67 ns (range -11.59%..-6.18%, interval
  -9.36%..-6.25%), comfortably passing its no-regression gate.
- Mechanism/code-size result: the literal arm now evaluates an ordinary array and
  enters the accepted direct array relocation path, removing repeated general push
  machinery and publishing length once. Complete `.text` shrank 336 bytes,
  `.rodata` was unchanged, and the executable shrank 8 bytes.
- Memory/semantic result: exact capacity and the existing direct-array allocation
  lifecycle remain one allocation, zero reallocations, one deallocation, and zero
  live bytes after drop. Empty and repeat arms are unchanged. Dedicated tests prove
  trailing-comma behavior, left-to-right evaluation, and exact-once cleanup of an
  initialized owning prefix when a later expression panics. All target/feature,
  no-std, Rust 1.86, Clippy, docs, and strict-provenance Tree Borrows Miri gates pass.
- Decision: retain the macro routing and semantic tests. Remove the temporary
  macro timing group; the permanent array benchmark already protects the distinct
  relocation mechanism.
- Artifact: `catalyzed-builder:~/thin-vec/benchmark-results/jackvec-exact-macro-20260710`.

### Transient construction builder (`perf/transient-builder`)

- Status: rejected; implementation and benchmark reverted
- Baseline commit: `b1bae7f`
- Hypothesis: final JackVec must keep length/capacity in its allocation header, but
  construction can hold both in a three-word transient owner and publish length
  only on growth, unwind, or `finish()`. Reusing the final prefix-header allocation
  from the start avoids moving elements during finalization while removing a header
  load/store from each successful no-growth push.
- API prototype: `JackVecBuilder<T>::new`, `with_capacity`, `push`, `len`,
  `capacity`, and consuming `finish() -> JackVec<T>`. Keep it opt-in and separate
  from final `JackVec<T>`; do not alter JackVec layout or ordinary push semantics.
- Primary CPU gate: preallocated construction and finish of 1,024 `u64` values on
  pinned Linux must improve at least 15% over preallocated JackVec push under seven
  paired rounds, cleared preload, seed 20260803, 100 samples, 3 s warm-up, 5 s
  measurement, and 100,000 resamples. Allocation/setup and output destruction stay
  outside timing; element writes, length handling, and final publication stay inside.
- Secondary sizes: 1 and 4 elements. Neither may regress more than 5%; report them
  even if the 1,024-element result passes. Compare whole executable and focused
  codegen so inlining does not hide an instruction-cache cost.
- Memory gate: final requested bytes, capacity, allocation count, and retained
  representation must exactly match JackVec constructed with the same capacity.
  Builder stack size is expected to be three words and must be reported, not counted
  as retained AST memory.
- Correctness gates: empty finish, preallocated and growing construction, ZST,
  over-alignment, owning exact-once drops on finish and unfinished-builder drop,
  capacity-overflow unwind after initialized values, Send/Sync behavior inherited
  from `T`, all feature/MSRV/docs/Clippy lanes, and strict-provenance Tree Borrows
  Miri.
- Scope: one builder type, one high-signal benchmark group with three declared
  sizes, and focused lifecycle tests. Do not combine inline scratch, exact small
  constructors, growth-policy changes, or sqlparsers integration.
- Candidate commit: `9e9279f` (reverted by `58b8e71`)
- CPU result: failed every declared size in fourteen same-binary process
  comparisons. Relative to ordinary preallocated JackVec construction, builder
  construction was 268.70% slower at one element (range +264.03%..+279.48%),
  135.19% slower at four (+94.20%..+141.24%), and 5.08% slower at 1,024
  (+4.53%..+5.39%). Median 1,024 estimates were 428.26 ns for JackVec and
  449.66 ns for the builder. The primary required a 15% improvement and both
  secondaries exceeded their 5% regression ceiling.
- Mechanism result: optimized assembly disproves the register-residency hypothesis.
  Builder `len` and `cap` remain recoverable stack fields because the cold growth
  call borrows the complete builder and Drop/unwind plus consuming finish must
  preserve ownership state. Each hot iteration reloads capacity and length and
  stores length back to the stack. Current optimized JackVec already carries loop
  length in a register and publishes only the required header update. Finish adds a
  singleton comparison/publication, explaining the particularly large fixed cost
  at one and four elements.
- Correctness result: the prototype itself passed empty/preallocated/growing, ZST,
  64-byte alignment, unfinished exact-once Drop, all feature/no-std/MSRV/Clippy/docs,
  and strict-provenance Tree Borrows Miri gates. Correctness does not rescue the
  failed performance premise.
- Decision: revert the public builder and its benchmark. A builder-only batch
  `extend` could keep loop state local, but accepted guarded `JackVec::extend`
  already provides that mechanism without another owner/API. Do not retry this
  three-word builder unless a different representation or downstream profile
  demonstrates a mechanism unavailable to ordinary JackVec.
- Artifact: `catalyzed-builder:~/thin-vec/benchmark-results/jackvec-builder-20260710`.

### Compact 32-bit allocation header (`perf/compact-header`)

- Status: accepted
- Baseline commit: `6b0f0ec`
- Hypothesis: JackVec's native header uses two machine words even though practical
  collection lengths fit in 32 bits. Replacing `{ len: usize, cap: usize }` with
  `{ len: u32, cap: u32 }` reduces every allocated header from 16 to 8 bytes on
  64-bit targets while preserving the one-word owner, singleton, Option niche,
  contiguous slice, and reconstructable allocation layout.
- Compatibility: intentionally cap length and capacity at `u32::MAX`. Requests or
  growth beyond that limit must fail deterministically before allocation or pointer
  arithmetic; no tagged wide-header fallback in this experiment.
- Primary memory gate: requested bytes for nonempty `u8` and `u64` JackVecs on a
  64-bit target must fall by exactly 8 bytes at capacities 1, 4, and 1,024, with
  unchanged allocation/reallocation/deallocation counts and zero live bytes after
  drop. Record allocator usable bytes separately later; do not equate requested-byte
  savings with RSS.
- Alignment caveat: for element alignment above 8, reduced header bytes may become
  padding rather than reduce total layout. Preserve and report this rather than
  claiming a universal eight-byte allocation saving.
- CPU/code-size gate: preallocated 1,024-element push on the pinned Linux host must
  not regress beyond the calibrated 1% envelope in seven paired rounds under the
  existing cleared-preload protocol. Compare focused codegen, complete ELF sections,
  and allocation output; a memory win does not excuse an unexplained hot-path loss.
- Correctness gates: boundary capacity rejection without attempting allocation,
  ZST length/capacity behavior, over-aligned layouts, owning exact-once drops,
  singleton behavior, one-word `JackVec`/`Option<JackVec>`, all feature/MSRV/docs/
  Clippy lanes, and strict-provenance Tree Borrows Miri.
- Scope: header field widths, checked accessors/conversions, focused boundary and
  layout tests, and only the allocation diagnostics necessary to prove requested
  bytes. Do not combine builder, growth-policy, pointer-tagging, or capacity-class
  changes.
- Memory result: passed exactly. The header is 8 bytes with 8-byte alignment.
  Reserved `u8` requested bytes are 9, 12, and 1,032 at capacities 1, 4, and
  1,024; reserved `u64` bytes are 16, 40, and 8,200. Each is exactly 8 bytes below
  the 16-byte-header baseline, with one allocation, no reallocations, one
  deallocation, and zero live bytes after drop. For 10,000 containers, sparse
  requested memory falls 176,000 -> 160,000 bytes and four-element memory falls
  560,000 -> 480,000; empty owner memory remains 80,000 because empties allocate
  nothing. Over-aligned layouts remain correct and may consume the saving as padding.
- First CPU falsification (`dbcf003`): failed decisively at +13.98%, with every
  round slower and `.text` +1,184 bytes. Removing the checked length conversion in
  `77c25cf` did not help (+14.80%), disproving the initial conversion-check theory.
- Root cause: changing two `usize` fields to `u32` reduced `Header`'s declared
  alignment from 8 to 4. For `u64`, `data_raw` could no longer prove the empty
  singleton's one-past-header pointer aligned, so optimized hot loops gained a
  capacity load/test, data-pointer `lea`, and conditional move on every push.
- Correction (`1e14cd8`): `repr(C, align(8))` keeps the header at 8 bytes while
  restoring the compile-time alignment proof for ordinary elements. The final
  seven-round push audit measured +0.50% (range +0.18%..+1.03%, interval
  +0.29%..+0.56%), inside the pre-registered 1% envelope. `.text` shrank 1,008
  bytes, `.rodata` grew 48 bytes, and the executable grew only 8 bytes. Record the
  half-percent point estimate as a possible small tradeoff, not a speedup or zero.
- Correctness result: the `u32::MAX` ZST boundary succeeds without allocation and
  the next capacity rejects before allocation on 64-bit. Updated maximum-range
  drain tests pass. All-target, no-default, serde, malloc-size, Rust 1.86,
  warning-denied Clippy/docs, 49 doctests, allocation/CPU smoke, strict-provenance
  Tree Borrows Miri, over-alignment, owner/Option niche, and diff gates pass.
- Artifacts: `catalyzed-builder:~/thin-vec/benchmark-results/jackvec-compact-header-20260710`,
  `.../jackvec-compact-header-corrected-20260710`, and
  `.../jackvec-compact-header-aligned-20260710`.

### JackVec API rename (`docs/jackvec-branding`)

- Status: accepted
- Naming contract: package and crate path `jackvec`, primary type `JackVec<T>`,
  construction macro `jack_vec!`.
- Compatibility: intentionally breaking. Do not retain aliases in the primary
  crate; a separate compatibility shim can be evaluated if an actual downstream
  migration requires one.
- Attribution: preserve explicit credit and links to Mozilla's `thin-vec`, its
  original authors, and contributors. Do not rewrite historical experiment names
  or results that were recorded against ThinVec.
- Scope: rename active source, doctests, tests, benchmarks, allocation labels, and
  benchmark tooling atomically. Repository-directory and remote URLs remain on the
  existing `tomsanbear/thin-vec` location until the repository itself is renamed.
- Acceptance: no stale `ThinVec`, `thin_vec`, or `thin-vec` API references outside
  intentional attribution/history/current repository URLs; all feature, Rust 1.86,
  docs, Clippy, Miri, macro-hygiene, allocation-smoke, and CPU-smoke gates pass;
  rename-only optimized executable code remains performance-equivalent.
- Validation result: the active source, tests, doctests, benchmarks, allocation
  output, and benchmark tooling contain only `jackvec`, `JackVec`, and `jack_vec!`.
  The remaining old-name matches are deliberate Mozilla attribution, historical
  records, or the current repository URL. All-target, no-default, serde,
  malloc-size, Rust 1.86, formatting, warning-denied Clippy, warning-denied docs,
  49 doctests, 99 strict-provenance Tree Borrows Miri tests, five benchmark-runner
  unit tests, allocation smoke, CPU smoke, and diff hygiene pass.
- Rename-only audit: a controlled Linux comparison used the same `jackvec`
  manifest, dependency graph, benchmark names, toolchain, allocator, and pinned CPU;
  the control exposed the pre-rename `ThinVec` implementation only through a local
  `JackVec` import alias. Preallocated 1,024-element push improved 3.65% in every
  paired round (range -4.38%..-2.67%, bootstrap interval -3.94%..-3.31%). The ELF
  `.text` and `.rodata` section sizes were identical, while instruction bytes and
  whole-file layout differed and the candidate file grew 40 bytes. Treat this as a
  favorable Rust/LLVM name-and-layout codegen side effect, not an algorithmic claim;
  the acceptance conclusion is strictly no rename regression.
- Rename audit artifact:
  `catalyzed-builder:~/thin-vec/benchmark-results/jackvec-rename-audit-retry-20260710`.

### Remove Gecko compatibility (`refactor/remove-gecko`)

- Status: accepted
- Decision: this fork targets native Rust and sqlparsers. Remove the `gecko-ffi`
  feature, nsTArray representation, external singleton linkage, Gecko growth policy,
  AutoThinVec support, Gecko-only tests, documentation, and CI lanes.
- Rationale: the alternate representation does not benefit the proving workload and
  forces every safety/performance change to preserve a second allocator, capacity,
  FFI, alignment, and stack-buffer model. Removing it makes the native one-word
  owner the only supported representation.
- Compatibility: intentionally breaking for users of `gecko-ffi`, `AutoThinVec`, or
  `auto_thin_vec!`. Native ThinVec, no-std, serde, malloc-size reporting, and
  benchmark APIs must remain intact.
- Acceptance: no Gecko configuration, public symbols, or implementation CFGs remain
  in source, manifest, README, or CI; native layout and Option niche remain one word;
  all native/no-default/serde/malloc/MSRV/docs/Clippy/Miri lanes pass; CPU and
  allocation smoke suites remain operational.
- Native audit: compare this branch against its exact parent (`2c6d9a0`) on the
  pinned Linux builder using `push_preallocated/ThinVec/1024`, 7 interleaved rounds,
  seed 20260728, glibc System allocator, 100 samples, 3 s warm-up, and 5 s
  measurement. Treat the removal as performance-neutral only if no repeatable
  regression above 1% appears; also compare optimized benchmark executable size.
- Validation result: source, manifest, README, and CI contain no current Gecko,
  AutoThinVec, nsTArray, or external-singleton surface. Default/all-target,
  no-default, serde, malloc-size, Rust 1.86, formatting, warning-denied Clippy,
  warning-denied docs plus 49 doctests, strict-provenance Tree Borrows Miri, and
  diff hygiene all pass. The remaining Gecko mentions in this ledger are preserved
  historical experiment evidence, not supported code or configuration.
- Initial Linux audit result: accepted as non-regressing. Across 7 paired
  rounds, `push_preallocated/ThinVec/1024` moved from 1,181.45 ns to 1,179.29 ns,
  a -0.38% paired median delta (100,000-resample bootstrap interval -0.66% to
  -0.02%; observed range -0.74% to +0.67%). The optimized CPU benchmark executable
  grew by only 8 bytes, from 4,030,312 to 4,030,320 bytes (+0.0002%). The runner's
  harness-difference guard was explicitly overridden after auditing that benches
  were identical and the manifest change only removed the unused `gecko-ffi`
  feature; both controlled-path digests are retained in the artifact manifest.
- Cross-platform causality audit: revise the CPU interpretation to **neutral**.
  The complete optimized machine-code section is byte-for-byte identical between
  baseline and candidate on both targets: macOS arm64 `__TEXT,__text` SHA-256
  `20bbff2d...d6ca2` (2,020,304 bytes) and Linux x86-64 `.text` SHA-256
  `f5467c69...37e0`. The whole macOS files differ only outside executable text,
  including their independently generated Mach-O `LC_UUID`; Linux's 8-byte file
  size difference is likewise outside `.text`. There is therefore no instruction
  change capable of causing a native CPU delta in this workload.
- macOS falsification: two baseline/candidate runs reported +4.85% and +1.55%
  candidate medians, but with respective paired ranges of -0.95%..+8.51% and
  -22.69%..+9.85%. An A/A proxy using commits differing only in `TODO.md` then
  reported a false -2.68% delta with a -13.50%..+12.95% range. The Apple M4 Max
  exposes 10 performance and 4 efficiency cores, macOS provides no equivalent to
  the Linux runner's exact CPU affinity, and measurements showed strong temporal
  speed drift independent of label. Aggregate load and memory/thermal pressure
  were healthy, so heterogeneous-core/frequency/scheduling placement—not source
  performance—is the supported explanation.
- Statistical correction: the Linux -0.38% interval must not be called a real gain.
  Its bootstrap resamples the seven observed pairs and cannot model label-independent
  temporal/system effects; identical `.text` is stronger causal evidence. Retain
  the experiment as proof of no regression, not proof of speedup.
- Artifact: `catalyzed-builder:~/thin-vec/benchmark-results/remove-gecko-native-audit-20260710`
- macOS artifacts: `benchmark-results/remove-gecko-native-audit-macos-20260710`,
  `benchmark-results/remove-gecko-native-audit-macos-repeat-20260710`, and
  `benchmark-results/remove-gecko-macos-aa-proxy-20260710`.
- Decision: retain the removal. It deletes 664 lines and the second allocator/ABI
  model without regressing the measured native hot path or materially changing
  optimized executable size.

### Gecko total-byte slow-growth threshold (`fix/gecko-growth-threshold`)

- Status: accepted
- Falsification commit: `ab460e8`
- Correction commit: `1656fcc`
- Hypothesis: Gecko reserve computes `min_cap_bytes` including header bytes, but
  selects slow growth with `min_cap > 8 MiB`, accidentally treating an element
  count as bytes. nsTArray's threshold is total requested allocation bytes. Requests
  just above 8 MiB therefore take power-of-two growth and jump to 16 MiB instead of
  slow-growth megabyte rounding to 9 MiB.
- Falsification boundary: for Gecko `ThinVec<u8>`, reserve capacity `8 MiB - 8`
  (header-inclusive request exactly 8 MiB) must produce capacity `8 MiB - 8`;
  requesting one more element must currently produce `16 MiB - 8`. The correction
  must instead produce `9 MiB - 8` for the second request while leaving the exact
  boundary unchanged.
- Acceptance: compare `min_cap_bytes` to the threshold, add only boundary capacity
  tests, preserve checked overflow/ZST behavior, and run Gecko/native/no-default,
  Rust 1.86, formatting, Clippy, and focused strict-provenance Miri. No CPU benchmark:
  this is a deterministic capacity/peak-memory and nsTArray-policy correction.
- Memory effect: the crossing test must reduce requested allocation from 16 MiB to
  9 MiB (a 7 MiB/43.75% reduction) with the same requested initialized capacity.
- Scope: do not alter first-allocation exactness, megabyte rounding, growth factor,
  native policy, AutoThinVec behavior, or the Gecko ABI/layout.
- Falsification result: confirmed exactly. The header-inclusive request one byte
  above 8 MiB produced capacity 16,777,208 (`16 MiB - 8`) rather than the required
  9,437,176 (`9 MiB - 8`); the exact 8 MiB boundary remained `8 MiB - 8`.
- Correction result: comparing the already-validated `min_cap_bytes` against 8 MiB
  preserves the exact boundary and changes the one-byte crossing to capacity
  9,437,176. Requested allocation falls from 16 MiB to 9 MiB: 7 MiB/43.75% less,
  with identical minimum capacity and one allocation.
- Validation: full native, no-default-feature, Gecko, and Rust 1.86 lanes pass;
  formatting and supported Clippy pass with only the three existing Gecko warnings;
  the boundary passes strict-provenance Gecko Miri. ZST, overflow, native growth,
  first-allocation policy, ABI, header layout, and AutoThinVec code are unchanged.
- Decision: retain the one-comparison correction and deterministic boundary test;
  no CPU benchmark is justified for a growth-policy memory fix.

### Direct array construction (`perf/from-array-bulk`)

- Status: accepted
- Baseline commit: `1875f0a`
- Candidate commit: `37bf4f9`
- Hypothesis: `From<[T; N]>` knows the exact initialized element count at compile
  time, but currently enters array iteration and generic collection. Allocating an
  exact ThinVec once and relocating the array in one ownership transfer should
  remove iterator, reserve, and per-element publication machinery while preserving
  compile-time arity information.
- Primary workload: construct a `ThinVec<u64>` from a four-element array. Array
  setup and returned ThinVec destruction are outside timing; ThinVec allocation,
  relocation, and length publication remain inside.
- Primary threshold: at least 10% faster at the paired median under cleared-preload
  System malloc, interval entirely below zero, and outside the 1% envelope. Seven
  rounds, seed `20260727`, CPU 0, sample size 100, 3-second warm-up, 5-second
  measurement, and 100,000 resamples.
- Memory/correctness gate: preserve one 48-byte requested allocation, no
  reallocations, exact capacity, empty and singleton arrays, owning exact-once
  drops, ZSTs, and native over-alignment. Run all feature/MSRV/Clippy lanes and
  focused native/Gecko strict-provenance Miri.
- Codegen/size gate: verify one allocation and direct fixed-size relocation without
  iterator fallback; measure focused and whole size. Reject a timing result caused
  by omitted destruction or materially larger generic monomorphization.
- Scope: change only `From<[T; N]>`, focused tests, and one temporary four-element
  benchmark. Do not add `one`/`two`/`from_fn` APIs or combine builder/Gecko work.
- Result: accepted. Four-element array construction improved 19.21%, from 14.02 ns
  to 11.28 ns, with every paired round favorable and a bootstrap interval of
  [-19.79%, -18.98%]. This clears the 10% threshold and calibrated noise envelope.
- Mechanism result: the baseline already coalesces the four values into two 128-bit
  loads/stores, but enters `ThinVec::reserve` from the singleton and retains generic
  iterator/reserve fallback state before publishing length. The candidate uses
  compile-time `N` to request the 48-byte layout directly, writes capacity, performs
  exactly two vector loads/stores, and publishes length once. No runtime iterator
  contract is trusted.
- Memory result: unchanged at one exact 48-byte requested allocation, zero
  reallocations, capacity four, and one deallocation. Array setup and output
  destruction remain outside the timed boundary in both builds.
- Size result: the focused Criterion wrapper shrank 25 bytes (1,763 to 1,738).
  Actual whole `.text` shrank 5,520 bytes and total `size` text shrank 6,200 bytes;
  the file shrank 12,136 bytes. Treat the large whole-binary reduction as favorable
  but LTO-sensitive and claim only the focused 25-byte change as local evidence.
- Correctness result: empty, singleton, four-element, owning exact-once drop, ZST,
  exact capacity, and native over-alignment cases pass. Native, no-default-feature,
  Gecko, supported Clippy, and focused native/Gecko strict-provenance Miri pass.
- Decision: retain direct array relocation and its one four-element benchmark.

### Boxed-slice delegation to direct inbound relocation (`perf/box-into-thin-delegate`)

- Status: rejected; implementation/test/temporary benchmark removed
- Baseline commit: `71cf4d8`
- First candidate commit: `3de8ff7`
- Hypothesis: `Box<[T]>` to `Vec<T>` transfers the same allocation without moving
  elements. Delegating immediately to the accepted direct Vec-to-ThinVec conversion
  should remove the current generic iterator collector and reproduce its bulk path
  without additional unsafe code or allocation.
- Primary workload: convert a preconstructed 1,024-element `Box<[u64]>` into
  `ThinVec<u64>`, with source clone/setup and output destruction outside timing.
- Primary threshold: at least 15% faster at the paired median under cleared-preload
  System malloc, interval entirely below zero, and outside the 1% envelope. Seven
  rounds, seed `20260725`, CPU 0, sample size 100, 3-second warm-up, 5-second
  measurement, and 100,000 resamples.
- Memory/correctness gate: preserve one source and one destination allocation, zero
  reallocations, exact output capacity, 16,400-byte requested peak at 1,024 `u64`s,
  empty/owning/ZST/over-aligned behavior, and exact once-only drops. Run all feature,
  MSRV, Clippy, and focused native/Gecko Miri lanes.
- Codegen/size gate: optimized code must reuse the direct Vec relocation after the
  allocation-free Box-to-Vec representation change. Inspect focused and whole size.
- Scope: one delegation change and one temporary high-signal CPU benchmark. Remove
  the benchmark after acceptance because the retained Vec-to-ThinVec lane protects
  the same mechanism; retain it only if Box introduces distinct generated behavior.
- First timing result: the delegation improved 31.04%, from 235.93 ns to 162.48 ns,
  with every round favorable and an interval of [-32.36%, -30.32%]. Allocation and
  semantic gates pass, so the mechanism is real.
- First size result: rejected. Actual ELF `.text` grew 1,856 bytes, `.rodata` 192,
  unwind/exception sections 128, and data-relocation storage 24; total `size` text
  grew 2,152 bytes even though the file shrank 216 bytes. The baseline has one
  434-byte outlined Box conversion. Delegation removes that symbol but inlines the
  accepted Vec relocation into multiple contexts: the timed wrapper grows 143 bytes
  and `cpu::main` grows 1,516. This is generalized duplication, not allocator noise
  or section-label confusion, and it vetoes the first candidate.
- Follow-up hypothesis: add only `#[inline(never)]` to the Box conversion method so
  Box-to-Vec remains allocation-free and the accepted relocation stays shared behind
  one call boundary. Compare exact baseline `71cf4d8` with the follow-up using seven
  rounds, seed `20260726`, and the same CPU/sample/warm-up/measurement/resample and
  cleared-preload settings. It must remain at least 15% faster with an interval
  below zero and actual ELF `.text` no larger than baseline. Otherwise revert the
  full delegation and remove its test/benchmark.
- Follow-up commit: `a6f24cd`
- Follow-up result: timing remained favorable at -31.96%, from 236.46 ns to 161.38
  ns, with interval [-32.23%, -28.93%]. Local outlining succeeded: the conversion
  helper shrank from 434 to 133 bytes and the timed wrapper exactly matched the
  baseline's 1,606 bytes. Allocation and correctness gates remained satisfied.
- Follow-up size result: rejected. Actual `.text` still grew 1,728 bytes, `.rodata`
  192, unwind/exception sections 192, and data-relocation storage 24; the file grew
  24 bytes. `cpu::main` remained 1,516 bytes larger and contained 314 calls versus
  288. Unrelated nested-workload setup was emitted six times rather than four, with
  16 extra indirect GOT calls. The attribute therefore removed local duplication
  but could not prevent the implementation change from perturbing LLVM/LTO's global
  inline/outline decisions.
- Decision: the explicit whole-text gate fails twice. Restore generic boxed-slice
  collection and remove the follow-up test and temporary benchmark. Preserve both
  fast timing results as evidence that delegation is a CPU win, but do not accept a
  downstream-generic code-size regression whose placement is compiler-sensitive.

### Direct `Vec<T>` to `ThinVec<T>` relocation (`perf/vec-into-thin-bulk`)

- Status: accepted
- Baseline commit: `b39ecf0`
- Candidate commit: `1f278f9`
- Hypothesis: current generic `Vec::IntoIter` collection already vectorizes its
  element movement, so only a direct experiment can distinguish equivalent bulk
  code from avoidable iterator/fallback overhead. One ThinVec allocation, one
  explicit bulk relocation, ownership publication, and source deallocation may let
  optimized libc movement outperform the compiler-generated loop.
- Primary workload: consume a preconstructed 1,024-element `Vec<u64>` into a
  `ThinVec<u64>`. Source cloning/setup and output destruction stay outside timing;
  destination allocation, relocation, source deallocation, and header publication
  remain inside.
- Primary threshold: at least 15% faster at the paired median under cleared-preload
  System malloc, bootstrap interval entirely below zero, and improvement outside the
  calibrated 1% envelope. Use one size because code inspection already established
  current bulk vectorization and this experiment asks only whether it remains a
  material hot-path cost.
- Fixed measurement parameters: seven paired rounds, seed `20260724`, CPU 0,
  Criterion sample size 100, 3-second warm-up, 5-second measurement, 100,000
  resamples, preload cleared, and label-neutral child paths.
- Memory/allocation gate: full construction plus conversion must preserve two
  allocations, zero reallocations, two deallocations, requested output bytes, peak
  live requested bytes, output capacity, and source release.
- Correctness gates: singleton and allocated empty inputs, spare source capacity,
  owning exact-once drops, ZSTs, and native over-alignment. Require all feature/MSRV
  lanes and focused native/Gecko strict-provenance Miri.
- Codegen/size gate: compare the existing vectorized collector with explicit bulk
  relocation and measure focused plus whole-ELF size. If timing is null or negative,
  identify whether equivalent copy code, allocator dominance, or a worse library
  call boundary explains it before reverting.
- Scope: change only `From<Vec<T>> for ThinVec<T>`, focused tests, and one temporary
  benchmark. Do not combine boxed input, clone, growth, or Gecko policy changes.
- Result: accepted. The 1,024-element conversion improved 25.22% at the paired
  median, from 215.48 ns to 161.14 ns. Every round was favorable; the bootstrap
  interval was [-28.33%, -24.17%], clearing the 15% threshold and noise envelope.
- Mechanism result: the baseline does move four `u64`s per vectorized iteration,
  but first enters generic reserve/iterator machinery, performs alias/distance and
  vectorization-threshold checks, retains a scalar tail plus reserve fallback, and
  carries iterator cleanup state. The candidate allocates the header once and calls
  optimized glibc `memcpy` for the 8 KiB region before one length publication and
  source deallocation. Thus the baseline was bulk-capable but not equivalent: tuned
  library movement plus removed control/unwind state explains the remaining win.
- Memory result: unchanged at two allocations, zero reallocations, two
  deallocations, 8,208 requested output bytes, and 16,400 peak live requested bytes.
  Output capacity remains exactly 1,024.
- Size result: the Criterion batched monomorph shrank 453 bytes (2,157 to 1,704).
  Whole-ELF text shrank 1,804 bytes, data shrank 32 bytes, and the executable file
  shrank 2,840 bytes.
- Correctness result: singleton and allocated empty inputs, spare capacity, owning
  exact-once drops, ZSTs, contents, exact capacity, and native over-alignment are
  covered. Native, no-default-feature, Gecko, Rust 1.86, formatting, supported
  Clippy, and focused native/Gecko strict-provenance Miri lanes pass. Gecko Clippy
  retains only its three existing warnings.
- Decision: retain the direct relocation and one high-signal benchmark. Boxed-slice
  input can reuse this now-proven primitive through its zero-cost Box-to-Vec
  conversion, but still requires an isolated end-to-end check.

### Direct `ThinVec<T>` to `Box<[T]>` relocation (`perf/thin-into-box-bulk`)

- Status: accepted
- Baseline commit: `41870bc`
- Candidate commit: `94dfb3b`
- Hypothesis: the current conversion collects `ThinVec::IntoIter` into a boxed
  slice through generic iterator machinery. Constructing one exact-length
  uninitialized boxed slice, relocating the initialized ThinVec region once,
  publishing box initialization, and emptying/deallocating the source should remove
  per-element checks and any intermediate Vec shrink path.
- Primary workload: consume a preconstructed 1,024-element `ThinVec<u64>` into a
  `Box<[u64]>`. Source cloning/setup and returned-box destruction stay outside
  timing; exact destination allocation, relocation, source-header deallocation,
  and initialization publication remain inside.
- Primary threshold: at least 15% faster at the paired median under cleared-preload
  System malloc, bootstrap interval entirely below zero, and improvement outside the
  calibrated 1% A/A envelope. Use one size only; the already-retained Vec conversion
  covers small bulk-ownership transfer and correctness tests cover Box edge cases.
- Fixed measurement parameters: seven paired rounds, seed `20260723`, CPU 0,
  Criterion sample size 100, 3-second warm-up, 5-second measurement, 100,000
  resamples, preload cleared, and label-neutral child paths. Do not extend or remove
  rounds.
- Memory/allocation gate: compare full source-build-plus-conversion allocation and
  deallocation counts, requested output bytes, peak live requested bytes, and exact
  output length. The candidate must neither retain the source nor allocate an
  intermediate Vec.
- Correctness gates: singleton and allocated empty inputs, singleton/nonempty and
  spare-capacity inputs, owning exact-once drops, ZSTs, and native over-alignment.
  Require native and Gecko strict-provenance Miri plus native, no-default-feature,
  Gecko, MSRV, formatting, and Clippy lanes.
- Codegen/size gate: confirm one exact destination allocation, one bulk relocation,
  initialization publication, and matching source deallocation without an
  element/capacity loop. Measure focused and whole-ELF size.
- Scope: change only `From<ThinVec<T>> for Box<[T]>`, focused tests, and one
  temporary benchmark shape. Do not combine inbound conversions, clone, growth,
  or Gecko policy changes.
- Result: accepted. The 1,024-element conversion improved 78.36% at the paired
  median, from 684.45 ns to 148.02 ns. Every paired round was favorable and the
  bootstrap interval was [-78.46%, -78.22%], clearing both the 15% threshold and
  calibrated noise envelope.
- Mechanism result: optimized code performs one exact 8,192-byte allocation, one
  `memcpy`, one source length reset, and one matching source-header deallocation,
  returning the pointer/length fat pointer directly. The old benchmark path retains
  generic iterator collection state and per-element movement/capacity control. The
  benchmark's `iter_batched` source setup and output destruction are unchanged and
  outside timing in both exact builds.
- Memory result: unchanged. Full construction plus conversion requests two
  allocations, zero reallocations, and two deallocations; output requested size is
  8,192 bytes and peak live requested bytes are 16,400. No intermediate allocation
  or retained source exists in the candidate.
- Size result: the direct boxed conversion monomorph is 253 bytes and its Criterion
  batched wrapper shrank 84 bytes. Whole-ELF text shrank 1,276 bytes, data shrank 24
  bytes, and the executable file shrank 4,464 bytes. Because global generic
  deduplication contributes to the larger whole-binary delta, claim only the wrapper
  reduction as focused evidence.
- Correctness result: singleton and allocated empty sources, spare capacity,
  contents, owning exact-once drops, ZSTs, and native over-alignment are covered.
  Native, no-default-feature, Gecko, Rust 1.86, formatting, supported Clippy, and
  focused native/Gecko strict-provenance Miri lanes pass. Gecko Clippy retains only
  the three existing legacy warnings.
- Decision: retain direct exact-box relocation and its single high-signal benchmark.

### Direct `ThinVec<T>` to `Vec<T>` relocation (`perf/thin-into-vec-bulk`)

- Status: accepted
- Baseline commit: `414df4a`
- Candidate commit: `0adc7b1`
- Hypothesis: the current `From<ThinVec<T>> for Vec<T>` delegates to generic
  `IntoIterator::collect`, which optimized x86-64 and AArch64 code still implements
  as a per-element move loop with capacity checks, reserve fallback, and unwind
  state. Allocating the destination once, relocating the initialized slice in bulk,
  publishing its length once, and emptying the source before deallocation should
  remove that work without changing the required two-layout ownership transfer.
- Primary workload: consume a preconstructed 1,024-element `ThinVec<u64>` into a
  `Vec<u64>`. Source cloning/setup stays outside timing; destination allocation,
  element relocation, source-header deallocation, and length publication remain
  inside. Destination destruction/deallocation stays outside timing.
- Primary threshold: at least 15% faster at the paired median under cleared-preload
  System malloc, bootstrap interval entirely below zero, and improvement outside the
  calibrated 1% A/A envelope.
- Declared secondary workload: the same conversion at four elements. It must not
  regress beyond the calibrated 1% envelope. There is no Vec identity control:
  moving a Vec into itself would omit the layout-changing allocation and is not
  semantically comparable. Decide only on exact-parent ThinVec A/B.
- Fixed measurement parameters: seven paired rounds, seed `20260722`, CPU 0,
  Criterion sample size 100, 3-second warm-up, 5-second measurement, 100,000
  resamples, preload cleared, and label-neutral child paths. Do not extend or remove
  rounds.
- Memory/allocation gate: compare allocation count, requested destination bytes,
  peak live requested bytes while both layouts coexist, final Vec capacity, source
  deallocation, and output contents. The candidate must not hide work by changing
  the timing boundary or retaining the source allocation.
- Correctness gates: singleton empty and allocated-empty sources, singleton and
  nonempty values, spare source capacity, owning values with exact once-only drops,
  ZSTs, and over-aligned elements. Require native and Gecko strict-provenance Miri
  plus native, no-default-feature, Gecko, MSRV, formatting, and Clippy lanes.
- Codegen/size gate: confirm one allocation, one bulk relocation, one length
  publication, and matching source deallocation with no per-element capacity loop.
  Measure the focused monomorphization and whole ELF; reject or explicitly record
  unexplained growth.
- Scope: change only `From<ThinVec<T>> for Vec<T>`, its focused tests, and two
  temporary benchmark sizes. Do not combine `Vec`/boxed-slice into ThinVec,
  ThinVec-to-boxed-slice, clone specialization, growth, or Gecko policy changes.
- Result: accepted. The 1,024-element conversion improved 80.64% at the paired
  median, from 674.85 ns to 130.65 ns, with every round favorable and a bootstrap
  interval of [-80.82%, -80.54%]. Four elements improved 9.29%, with an interval
  of [-9.97%, -9.01%], so the secondary no-regression gate also passes.
- Mechanism result: exact disassembly shows the 648-byte baseline collector moving
  one value per iteration while updating iterator position, checking destination
  capacity, retaining a reserve fallback, and carrying unwind cleanup. The
  278-byte candidate performs one destination allocation, one `memcpy`, one source
  length reset, one destination length publication, and one matching source-header
  deallocation. This is the pre-registered mechanism, and setup/output destruction
  remain on the same sides of the timing boundary in both exact builds.
- Memory result: unchanged. Full source-build-plus-conversion lifecycle uses two
  allocations, zero reallocations, and two deallocations. Requested peak is 80
  bytes at four elements and 16,400 bytes at 1,024; returned Vec requested bytes
  are 32 and 8,192 respectively, with exact capacity. The candidate neither omits
  the necessary second layout nor retains the source allocation.
- Size result: focused conversion code shrank 370 bytes (648 to 278). Whole-ELF
  text shrank 580 bytes, data was unchanged, and the executable file shrank 104
  bytes.
- Correctness result: singleton and allocated empty inputs, spare capacity, owning
  exact-once drops, ZSTs, native over-alignment, contents, and destination capacity
  are covered. Native, no-default-feature, Gecko, Rust 1.86, formatting, supported
  Clippy, and focused native/Gecko strict-provenance Miri lanes pass. Gecko Clippy
  reports only the three previously existing legacy warnings.
- Decision: retain the direct relocation and its two-size CPU/allocation coverage.
  The same ownership-transfer primitive is promising for ThinVec-to-boxed-slice,
  but that direction remains a separate experiment because exact-capacity Box
  construction and codegen differ.

### Nonempty clone outlining policy (`perf/clone-inlining`)

- Status: rejected; candidate and permanent benchmark removed
- Baseline commit: `a523aa3`
- Candidate commit: `0698f97`
- Hypothesis: marking every nonempty clone path `cold` and `inline(never)` imposes an
  unnecessary call boundary on ordinary small clones. Allowing normal inlining may
  improve four-element clone-and-drop, but can increase downstream code size.
- Primary workload: clone and destroy a preconstructed four-element `u64`
  collection. Source construction stays outside timing; allocation, element copy,
  length publication, and clone destruction/deallocation remain inside.
- Primary threshold: at least 10% faster at the paired median under cleared-preload
  System malloc, bootstrap interval entirely below zero, and improvement outside the
  calibrated 1% A/A envelope.
- Declared secondary workload: the same full lifecycle for 1,024 `u64` elements. It
  must not regress beyond the calibrated 1% envelope. Report matching Vec controls,
  but decide on exact-parent ThinVec A/B.
- Fixed measurement parameters: seven paired rounds, seed `20260720`, CPU 0,
  Criterion sample size 100, 3-second warm-up, 5-second measurement, 100,000
  resamples, preload cleared, and label-neutral child paths. Do not extend or remove
  rounds.
- Correctness gates: unchanged empty singleton cloning, disjoint nonempty storage,
  exact contents/capacity behavior, ZST and owning values, and the existing
  partial-initialization clone-panic test. Require native and Gecko
  strict-provenance Miri and all feature/MSRV lanes.
- Codegen/size gate: inspect focused small and large monomorphizations plus whole
  ELF. Reject a small timing win if normal inlining materially duplicates clone,
  allocation, or unwind machinery; record any accepted tradeoff explicitly.
- Scope: change only clone outlining/inlining attributes and add two benchmark
  sizes. Do not change the partial-initialization algorithm, conversions, allocation
  growth, or element cloning semantics.
- Result: rejected despite clearing the primary small-clone gate. Four-element
  ThinVec clone-and-drop improved 12.92% at the paired median, with a bootstrap
  interval of [-15.72%, -12.30%], but the declared 1,024-element secondary regressed
  4.17%, with every paired round unfavorable and an interval of [+3.57%, +4.50%].
  The secondary regression exceeds the calibrated 1% envelope and therefore vetoes
  the candidate. Vec controls were neutral: -0.01% at four elements and -0.18% at
  1,024, with both intervals spanning zero.
- Codegen explanation: the outlined baseline clone loop is 32-byte aligned and its
  31-byte vectorized copy loop fits in one fetch block. Inlining duplicates that
  loop inside the Criterion iteration body at a 16-byte offset, so the same 31-byte
  loop straddles a 32-byte fetch boundary. At 1,024 elements both versions execute
  256 iterations of the same two-load/two-store vector body; the saved call/return
  is amortized while the less favorable hot-loop placement repeats. At four
  elements, inlining separately lowers LLVM's vectorization threshold: the
  candidate performs two 128-bit loads and stores, whereas the outlined baseline
  uses four scalar iterations. This accounts for the real small win and opposing
  large regression without appealing to allocator noise or omitted work.
- Alignment falsification: a separate three-round diagnostic rebuilt both exact
  commits with LLVM's preferred innermost-loop alignment forced to 32 bytes. The
  candidate loop moved from address offset 16 to a 32-byte boundary and the
  1,024-element delta collapsed from +4.17% to +0.01% (range -2.37% to +0.17%).
  This run is intentionally too short and globally alignment-perturbed for an
  acceptance claim, but it falsifies a semantic-work or allocation explanation and
  supports fetch placement as the cause of the original repeatable regression.
- Size result: whole-ELF text shrank 16 bytes and the file shrank 152 bytes, so total
  binary growth is not the regression mechanism. The measured Criterion iteration
  monomorph grows from 256 to 478 bytes after absorbing the 185-byte clone helper;
  its combined local footprint is 37 bytes larger than the outlined pair.
- Decision: preserve `cold`/`inline(never)` for the generalized clone path and
  remove the temporary benchmark. A future small-clone specialization would be a
  distinct situational experiment and must retain an outlined large path rather
  than repeating this global inlining policy.

### Bulk tail destruction in `truncate` (`perf/truncate-bulk-drop`)

- Status: rejected; candidate and permanent benchmark removed
- Baseline commit: `142cfe2`
- Candidate commit: `2db50d6`
- Hypothesis: publishing final length once and dropping the removed suffix as one
  slice will eliminate a header length load/store per destroyed element and allow
  normal slice drop glue to manage unwinding.
- Semantic caveat: current ThinVec truncation destroys from the old end backward;
  Rust 1.86 Vec destroys the removed suffix forward after setting final length.
  Destruction order is observable even though ThinVec does not document a guarantee.
  Any candidate must explicitly adopt and test Vec's forward order; do not describe
  this as mechanically behavior-preserving.
- Primary workload: truncate 1,024 preconstructed cheap owning/drop values to zero.
  Allocation and construction stay outside the timed region; destructor calls and
  length publication remain inside. Final vector destruction is empty.
- Primary threshold: at least 10% faster at the paired median under cleared-preload
  System malloc, bootstrap interval entirely below zero, and improvement outside the
  calibrated 1% A/A envelope. Report Vec as a link-layout/noise control, but decide
  on exact-parent ThinVec A/B.
- Fixed measurement parameters: seven paired rounds, seed `20260719`, CPU 0,
  Criterion sample size 100, 3-second warm-up, 5-second measurement, 100,000
  resamples, preload cleared, and label-neutral child paths. Do not extend or remove
  rounds.
- Correctness gates: no-op when requested length is equal/greater, partial and full
  truncation, forward destruction order, ZSTs with Drop, and a destructor panic in
  the removed suffix. Final length must already be published; every other removed
  and retained owning element must be destroyed exactly once. Require native and
  Gecko strict-provenance Miri and all feature/MSRV lanes.
- Codegen/size gate: confirm one header publication and slice drop rather than a
  header-updating loop, with no unexplained hot-function or whole-binary growth.
  Reject or record any tradeoff.
- Scope: one truncate implementation and one high-signal benchmark shape. Do not
  combine `clear`, clone inlining, extension, or Gecko growth changes.
- Result: rejected. The candidate improved 0.65% at the paired median, with a
  bootstrap interval of [-0.68%, -0.61%]. Although directional, this is inside the
  calibrated 1% envelope and far below the pre-registered 10% threshold. Unchanged
  Vec was neutral at +0.02%, with an interval spanning zero.
- Code-size result: whole-ELF text grew 412 bytes, data was unchanged, and the file
  grew 4,320 bytes. This cost and the observable reverse-to-forward destruction-order
  change are not justified by a sub-1% timing effect.
- Correctness result: all feature/test, Clippy, formatting, and focused native and
  Gecko strict-provenance Miri gates passed. Slice drop glue correctly finished the
  removed suffix after one destructor panicked. Correctness did not override the
  failed performance and semantic-risk gates.
- Mechanism explanation: retained disassembly confirms the baseline performs one
  header length load/store and walks backward for every removed value, while the
  candidate publishes length once and walks forward. Both execute exactly 1,024
  calls to the deliberately non-inlined cheap destructor. The hot header remains in
  L1 and its stores retire under destructor-call latency, so they are not the
  throughput bottleneck. Slice-drop unwind machinery also explains the candidate's
  larger code. The null result therefore reflects non-critical removed work, not a
  failed transformation or benchmark boundary error.
- Decision: restore the original reverse, per-element length-publishing truncate
  implementation and remove the temporary permanent benchmark. Preserve the raw
  remote artifacts and this negative result so the idea is not repeated without a
  materially different mechanism or workload.

### Guarded reserved `resize` growth (`perf/resize-guard`)

- Status: accepted
- Baseline commit: `5acd1a4`
- Candidate commit: `d1e1e42`
- Hypothesis: after `resize` reserves the full growth, cloning directly into the
  uninitialized suffix while carrying length in a panic guard will eliminate every
  repeated public `push` capacity check and header length publication. The final
  supplied value can still be moved rather than cloned.
- Primary workload: resize an empty `u64` collection with capacity for 1,024 to
  length 1,024. Allocation/setup and final destruction stay outside the timed
  region; cloning/copying, writes, and final length publication remain inside.
- Primary threshold: at least 15% faster at the paired median under cleared-preload
  System malloc, bootstrap interval entirely below zero, and improvement outside the
  calibrated 1% A/A envelope. Report Vec as a link-layout/noise control, but decide
  on exact-parent ThinVec A/B.
- Fixed measurement parameters: seven paired rounds, seed `20260718`, CPU 0,
  Criterion sample size 100, 3-second warm-up, 5-second measurement, 100,000
  resamples, preload cleared, and label-neutral child paths. Do not extend or remove
  rounds.
- Correctness gates: growth from empty and nonempty vectors, no-op equal length,
  shrinking through truncate, ZSTs, exact final values, clone panic after partial
  suffix initialization, and exact once-only destruction of original and cloned
  values. Require native and Gecko strict-provenance Miri and all feature/MSRV lanes.
- Codegen/size gate: confirm the reserved growth loop carries initialized length in
  registers and publishes it once, with no unexplained hot-function or whole-binary
  growth. Reject or record any tradeoff.
- Scope: one guarded resize growth loop and one high-signal benchmark size. Preserve
  move-the-last-value behavior; do not combine `extend_from_slice`, clone inlining,
  truncate, or Gecko growth changes.
- Result: accepted. Against exact parent `5acd1a4`, resizing reserved capacity to
  1,024 `u64` values improved 27.17% at the paired median, with a bootstrap interval
  of [-27.38%, -26.96%]. This clears the 15% gate and calibrated 1% envelope.
- Control result: unchanged Vec measured +0.37%, with an interval of
  [+0.18%, +0.97%]. This is directional but remains inside the pre-registered 1%
  envelope; one round reached +8.36%, so preserve the full range as an outlier.
- Codegen/size result: the focused Criterion monomorphization shrank 113 bytes, from
  0x609 to 0x598. The old loop retains a capacity branch and publishes header length
  after every cloned value. The candidate broadcasts the scalar value, vectorizes
  four suffix stores per iteration, carries initialized length locally, and
  publishes it once. Whole-ELF text grew 440 bytes, data was unchanged, and the file
  grew 176 bytes; record this as unwind/general-code growth despite the smaller hot
  monomorphization.
- Correctness result: native, no-default-feature, and Gecko test lanes pass;
  formatting passes; supported Clippy lanes add no warnings; focused native and
  Gecko strict-provenance Miri passes. Normal grow/no-op/shrink, nonempty vectors,
  ZSTs, and clone panic after two initialized suffix values are covered with exact
  once-only destruction.

### Clone partial-initialization guard (`fix/clone-panic-guard`)

- Status: accepted
- Baseline commit: `68f68be`
- Falsification test commit: `8cf7b55`
- Candidate commit: `e71f84f`
- Defect hypothesis: nonempty `ThinVec::clone` writes cloned elements into a new
  allocation while its published length remains zero. If a later `T::clone`
  panics, unwinding deallocates the buffer without dropping the earlier successful
  clones.
- Falsification: a four-element owning source whose third clone panics must show
  that the first two successful clones are not dropped on exact parent `68f68be`.
  The source elements must remain intact and each must still be dropped exactly
  once when the source is destroyed.
- Correction: track initialized length in a guard that publishes it during normal
  return or unwinding, so ordinary ThinVec destruction drops the initialized clone
  prefix and deallocates with the matching layout.
- Acceptance: the falsification test fails for the predicted clone-drop counts on
  the exact parent and passes after the guard; all native, no-default-feature,
  Gecko, Clippy, formatting, and focused strict-provenance Miri lanes pass.
- Scope: one partial-initialization guard and one owning panic test. Do not alter
  `cold`/`inline(never)`, add a clone benchmark, or combine conversion/Extend work.
  Performance policy for nonempty clone remains a later isolated experiment.
- Falsification result: confirmed exactly. On parent behavior, the first two
  successfully created clones both had drop count zero after the third clone
  panicked, while every source-element drop count remained zero. The test failed
  with observed clone counts `[0, 0, 0, 0]` versus required `[1, 1, 0, 0]`.
- Correction result: the guard publishes the initialized clone prefix during
  normal return or unwinding. The test now observes `[1, 1, 0, 0]`; source elements
  remain intact through the failed clone and each drops exactly once afterward.
- Validation: all native, no-default-feature, and Gecko test lanes pass; formatting
  passes; supported Clippy lanes add no warnings; focused native and Gecko
  strict-provenance Miri passes. No performance claim is made and the existing
  nonempty clone outlining attributes remain unchanged.

### Guarded reserved `Extend` (`perf/extend-guard`)

- Status: accepted
- Baseline commit: `f69629b`
- Candidate commit: `0b818d7`
- Hypothesis: while consuming the iterator's reserved lower-bound portion, keeping
  initialized length locally and publishing it once through a panic guard will
  remove a header length load/store per element without weakening safety. The
  lower bound limits the number of direct writes; it is never trusted as an exact
  length or as an unsafe iterator contract.
- Primary workload: extend an empty `u64` collection with capacity for 1,024 from
  `0..1_024`. Allocation/setup and final destruction stay outside the timed region;
  iterator creation, iteration, writes, and length publication remain inside.
- Primary threshold: at least 15% faster at the paired median under cleared-preload
  System malloc, bootstrap interval entirely below zero, and improvement outside the
  calibrated 1% A/A envelope. Report Vec as a link-layout/noise control, but decide
  on exact-parent ThinVec A/B.
- Fixed measurement parameters: seven paired rounds, seed `20260717`, CPU 0,
  Criterion sample size 100, 3-second warm-up, 5-second measurement, 100,000
  resamples, preload cleared, and label-neutral child paths. Do not extend or remove
  rounds.
- Correctness gates: existing nonempty destination, empty iterator, exact lower
  bound, overestimated and underestimated lower bounds, iterator panic after partial
  initialization, owning elements destroyed once, and ZST behavior. Require native
  and Gecko strict-provenance Miri and all supported feature/MSRV lanes.
- Codegen/size gate: confirm the reserved loop carries length in a register and
  publishes it once on the non-panicking path, with no unexplained hot-function or
  whole-binary growth. Reject or record any tradeoff.
- Scope: one guarded lower-bound loop and one high-signal benchmark size. Keep the
  existing checked `push` fallback for iterator items beyond the reserved portion;
  do not combine `resize`, `extend_from_slice`, clone, or truncate changes.
- Result: accepted. Against exact parent `f69629b`, reserved extension of 1,024
  `u64` values improved 24.72% at the paired median, with a bootstrap interval of
  [-25.12%, -24.36%]. This clears the 15% gate and calibrated 1% envelope.
- Control result: unchanged Vec measured -0.07% at the paired median and its
  bootstrap interval spanned zero at [-0.12%, +0.65%]. One round reached +9.29%,
  so retain the full range rather than describing the control as uniformly quiet.
- Codegen/size result: the focused Criterion monomorphization shrank 80 bytes, from
  0x5f5 to 0x5a5. The old unrolled loop stores header length after every element;
  the candidate vectorizes four range values per iteration, carries initialized
  length in registers, and stores the header length once after the loop. Whole-ELF
  text shrank 64 bytes, data was unchanged, and the file grew 32 bytes from section
  layout/alignment rather than loadable code or data.
- Correctness result: native, no-default-feature, and Gecko test lanes pass;
  formatting passes; supported Clippy lanes add no warnings; focused native and
  Gecko strict-provenance Miri passes. Exact, overestimated, and underestimated
  lower bounds, empty input, nonempty destination, ZSTs, and iterator panic after
  partial owning-element initialization are covered. The obsolete private
  `push_unchecked` helper was removed instead of retaining dead code.

### Guarded `dedup_by` backshift (`perf/dedup-backshift`)

- Status: accepted
- Baseline commit: `84ebc41`
- Candidate commit: `ea5969f` (test-only ZST follow-up: `22fa8cf`)
- Hypothesis: after the first adjacent duplicate, dropping duplicates in place and
  copying each later survivor once into the gap will outperform swapping each
  survivor with a duplicate, especially for large elements. A gap guard will repair
  the initialized prefix if the comparator or a duplicate's destructor panics.
- Primary workload: 256 64-byte `[u64; 8]` elements arranged as adjacent pairs,
  leaving 128 survivors. Setup/allocation and final destruction stay outside the
  timed region; comparison, duplicate destruction, and movement remain inside.
- Primary threshold: at least 15% faster at the paired median under cleared-preload
  System malloc, bootstrap interval entirely below zero, and improvement outside the
  calibrated 1% A/A envelope.
- Declared secondary workload: 1,024 `u64` elements in adjacent pairs, leaving 512
  survivors. It must not regress beyond the calibrated 1% envelope. Retain Vec as a
  link-layout/noise control, but decide on exact-parent ThinVec A/B.
- Fixed measurement parameters: seven paired rounds, seed `20260716`, CPU 0,
  Criterion sample size 100, 3-second warm-up, 5-second measurement, 100,000
  resamples, preload cleared, and label-neutral child paths. Do not extend or remove
  rounds.
- Correctness gates: stable first-of-run semantics, comparator argument order and
  mutation behavior, exact survivor values, owning elements destroyed once,
  comparator-panic repair, duplicate-destructor-panic repair, empty/singleton/
  all-unique/all-duplicate/ZST behavior, native and Gecko strict-provenance Miri,
  and all supported feature/MSRV lanes.
- Codegen/size gate: confirm one copy per post-gap survivor and no unexplained
  hot-function or whole-binary growth. Reject or record any tradeoff.
- Scope: one guarded algorithm, two high-signal benchmark shapes, and focused
  correctness tests. Do not combine `truncate`, `Extend`, or clone changes.
- Result: accepted. Against exact parent `84ebc41`, the 64-byte primary workload
  improved 25.49% at the paired median, with a bootstrap interval of
  [-25.55%, -25.42%]. The `u64` secondary improved 17.27%, with an interval of
  [-17.49%, -16.70%]. Both effects clear their gates and the calibrated 1%
  envelope.
- Control caveat: unchanged `Vec<u64>` measured 5.73% faster, while unchanged
  `Vec<[u64; 8]>` measured 3.73% slower; both intervals excluded zero. All five
  emitted `Vec::dedup_by` symbol sizes match between the two binaries. The
  opposite-direction type-specific shifts are consistent with the link-layout
  sensitivity seen in the retain experiment and cannot explain both ThinVec
  workloads improving. Preserve the raw controls and do not present them as a
  clean same-binary null result.
- Codegen/size result: the candidate emits 129-byte (`u64`) and 198-byte (64-byte
  element) ThinVec dedup monomorphizations. The 64-byte survivor path performs four
  16-byte loads and stores directly into the gap, without writing the displaced
  duplicate back to the source. Whole-ELF text shrank 1,012 bytes, data shrank 24
  bytes, and the file shrank 1,016 bytes.
- Correctness result: all native, no-default-feature, and Gecko test lanes pass;
  formatting passes; supported Clippy lanes add no warnings; focused native and
  Gecko strict-provenance Miri passes. Comparator and duplicate-destructor unwinds
  repair the gap and drop every owning element exactly once. Comparator argument
  order/mutation and empty, singleton, unique, duplicate, and mixed-copy ZST cases
  are covered.

### Guarded `retain_mut` backshift (`perf/retain-backshift`)

- Status: accepted
- Baseline commit: `fdb889d`
- Hypothesis: after the first rejection, moving each retained element once into the
  earliest hole will outperform `swap`, which moves both retained and rejected
  values, especially for large elements. A length-zero backshift guard will preserve
  a valid contiguous vector if the predicate or a rejected element's destructor
  panics.
- Primary workload: 256 64-byte `[u64; 8]` elements with alternating rejection.
  Setup/allocation and destruction of retained elements stay outside the timed
  region; predicate, rejected-element destruction, and movement remain inside.
- Primary threshold: at least 15% faster at the paired median under cleared-preload
  System malloc, bootstrap interval entirely below zero, and improvement outside the
  calibrated 1% A/A envelope.
- Declared secondary workload: 1,024 `u64` elements with alternating rejection. It
  must not regress beyond the calibrated 1% envelope. Report both Vec and ThinVec in
  the permanent comparison, but use exact-parent ThinVec A/B for the decision.
- Fixed measurement parameters: seven paired rounds, seed `20260715`, CPU 0,
  Criterion sample size 100, 3-second warm-up, 5-second measurement, 100,000
  resamples, preload cleared, and label-neutral child paths. Do not extend or remove
  rounds.
- Correctness gates: stable order, exact retained values, owning elements destroyed
  once, predicate-panic repair, rejected-destructor-panic repair, empty/all-kept/
  all-rejected/ZST behavior, native and Gecko strict-provenance Miri, and all
  supported feature/MSRV lanes.
- Codegen/size gate: confirm one copy per retained post-hole element and no
  unexplained hot-function or whole-binary growth. Reject or record any tradeoff.
- Scope: one guarded algorithm, two high-signal benchmark shapes, and focused
  correctness tests. Do not combine `dedup_by` or `truncate` changes.
- Result: accepted. Against exact parent `fdb889d`, the 64-byte primary workload
  improved 39.77% at the paired median, with a bootstrap interval of
  [-41.07%, -38.63%]. The `u64` secondary improved 29.79%, with an interval of
  [-30.00%, -28.10%]. Both effects are far outside the calibrated 1% envelope.
- Control caveat: unchanged `Vec<u64>` was neutral at -0.22%, with an interval
  spanning zero. Unchanged `Vec<[u64; 8]>` was 2.87% slower, with an interval of
  [+1.33%, +3.68%]. Its retained `Vec::retain_mut` machine-code sequence and
  0x287-byte symbol size are unchanged apart from relocation targets, but its
  address/alignment moved with the candidate binary. Record this as unresolved
  link-layout sensitivity; it is too small and opposite in direction to explain
  the 39.77% ThinVec improvement, but it prevents treating the control as a clean
  same-binary null result.
- Codegen/size result: the candidate emits separate 173-byte (`u64`) and 213-byte
  (64-byte element) ThinVec retain monomorphizations. The hot loops copy each
  post-hole survivor once; the 64-byte form uses four 16-byte vector loads/stores
  rather than swapping a survivor with a rejected value. Whole-ELF text grew
  1,260 bytes, data shrank 40 bytes, and the file grew 6,232 bytes. Retain this
  explicit code-size cost because the measured CPU wins are large.
- Correctness result: native, no-default-feature, and Gecko test lanes pass;
  formatting passes; supported Clippy lanes add no warnings; focused native and
  Gecko strict-provenance Miri passes. Predicate and rejected-destructor unwinds
  repair the hole, preserve the untouched suffix, and drop every owning element
  exactly once. Empty, all-kept, all-rejected, and ZST cases are covered.

### Splice reserve accounting (`perf/splice-reserve`)

- Status: accepted
- Baseline commit: `bafdc44`
- Hypothesis: `Drain::move_tail` double-counts the initialized prefix because it
  passes `end + tail + additional` to `ThinVec::reserve`, whose argument is already
  interpreted relative to current `vec.len() == end`.
- Proposed correction: reserve `tail + additional`; the reserve method then checks
  exactly `end + tail + additional`, the required post-move initialized layout.
- Final regression metric: resulting capacity for a five-element vector splicing two
  elements into eleven. Final length and minimum required capacity are 14. The exact
  parent produces capacity 18 natively and 30 in Gecko; the correction must produce
  14 in both lanes.
- Acceptance: exact final contents and length, capacity 14, no additional benchmark,
  unchanged behavior when replacement is shorter/equal, checked overflow retained,
  and all relevant ownership, ZST, native, no_std, Gecko, MSRV, Clippy, and focused
  strict-provenance Miri gates pass.
- Falsification: if the test does not fail at capacity 19 before implementation, or
  if allocator/growth policy rather than prefix double-counting explains the result,
  stop and revise the hypothesis instead of applying the arithmetic change.
- Scope: one regression test and the smallest `move_tail` arithmetic correction. No
  Criterion benchmark or unrelated splice rewrite.
- Native falsification result: confirmed the original prediction exactly. Before the
  correction, final contents/length were correct but capacity was 19 rather than 15;
  after the correction it is 15.
- Gecko prediction correction: the 15-element case remains capacity 30 even with
  corrected accounting because its 16-byte header makes the request 136 bytes and
  Gecko rounds it to 256. The original prediction of capacity 15 was false.
- Pre-registered Gecko boundary follow-up: replace the same two elements with eleven
  rather than twelve. Final length 14 plus the 16-byte header is exactly 128 bytes;
  corrected accounting predicts capacity 14, while prefix double-counting predicts
  a 256-byte allocation and capacity 30. Use this boundary for the shared regression
  test and report both native and Gecko results.
- Boundary result: confirmed in a separate parent worktree. The old implementation
  produced capacity 18 natively and 30 with Gecko; both corrected lanes produce
  capacity 14 with identical final length and contents.
- Implementation: assert the `vec.len() == end` invariant in debug builds, compute
  checked additional capacity as `tail + additional`, and let `reserve` add the
  current initialized prefix exactly once. Existing checked overflow behavior is
  preserved.
- Validation: full native and no-default-feature tests pass; all Gecko library tests
  pass; focused strict-provenance Miri passes natively and with Gecko. Supported
  stable Clippy lanes complete with only existing unrelated warnings. Gecko doctests
  remain non-linkable without the external `sEmptyTArrayHeader` fixture, as before.
- Decision: retain. This is a deterministic capacity/memory correction with no new
  timing benchmark and no semantic or ownership change.

### Paired A/B runner and same-binary calibration (`benchmarks/ab-runner`)

- Status: first remote calibration rejected
- Infrastructure hypothesis: exact-commit detached worktrees, a shared lockfile,
  identical benchmark sources, build-before-measurement, alternating process order,
  and retained raw artifacts remove the mutable-baseline and time-based counter
  errors already identified.
- Calibration hypothesis: independently built copies of commit `c868598` will
  produce byte-identical benchmark executables after source-path remapping and show
  no directional difference for `push_preallocated/ThinVec/1024` on the pinned host.
- Primary calibration metric: absolute median paired A/A wall-time delta across
  seven process rounds.
- Pre-registered command parameters: baseline and candidate `c868598`, exact filter
  `push_preallocated/ThinVec/1024`, seed `20260710`, CPU 0, Criterion sample size
  100, 3-second warm-up, 5-second measurement, and 100,000 resamples.
- Fixed stopping rule: exactly seven paired rounds. Do not extend or selectively
  remove rounds after inspecting their values.
- Calibration success: binaries are byte-identical, absolute median paired delta is
  at most 1%, the deterministic bootstrap interval includes zero, and every declared
  round is retained and reported. A failure qualifies the host/runner for further
  diagnosis, not an implementation performance conclusion.
- Secondary checks: per-round delta spread, run order, toolchain and host metadata,
  governor, raw Criterion samples, build logs, and zero missing/failed runs.
- Expected result: no effect. Any apparent directional win is evidence of measurement
  bias or instability because both labels contain identical code.
- Result: rejected despite the small absolute median. The two binaries were
  byte-identical, but the candidate label measured 0.119% faster at the median;
  paired deltas ranged from -0.522% to +0.365%, and the deterministic bootstrap
  interval was entirely negative at [-0.334%, -0.068%]. It therefore failed the
  pre-registered requirement that the interval include zero.
- Confounder discovered: the remote login environment globally injects
  `/usr/lib/x86_64-linux-gnu/libtcmalloc_minimal.so.4` through `LD_PRELOAD`. Prior
  results from this host must not be described as glibc-System-allocator results
  until they are rerun with preload explicitly cleared.
- Possible label mechanism under test: separately located executable files may
  carry stable loader, inode, or page-cache effects even when their bytes match.
  This is a hypothesis, not an explanation established by the failed run.

### Controlled same-binary recalibration

- Status: rejected
- Change from the rejected calibration: stage the selected build at one canonical
  executable pathname/inode before every invocation and explicitly clear inherited
  preload variables from build and benchmark children. Record both inherited and
  effective environments.
- Calibration hypothesis: with those two label-specific/environmental confounders
  controlled, identical commit `c868598` will show no directional difference for
  `push_preallocated/ThinVec/1024`.
- Primary metric and success rule: unchanged—absolute median paired delta at most
  1%, deterministic bootstrap interval includes zero, byte-identical builds, and
  all seven declared rounds retained.
- Pre-registered command parameters: baseline and candidate `c868598`, exact filter
  `push_preallocated/ThinVec/1024`, seed `20260711`, CPU 0, preload cleared,
  Criterion sample size 100, 3-second warm-up, 5-second measurement, and 100,000
  resamples.
- Fixed stopping rule: exactly seven paired rounds with no post-hoc removal or
  extension. Expected result: no effect; failure blocks optimization revalidation.
- Result: rejected. Despite identical executable hash, device, inode, and pathname,
  the candidate label measured 0.537% faster at the median; paired deltas ranged
  from -0.932% to +0.085%, and the bootstrap interval again excluded zero at
  [-0.803%, -0.345%].
- Allocator sensitivity discovered: clearing inherited tcmalloc changed the absolute
  estimate from roughly 440 ns to 1,130 ns. Even though allocation is outside the
  intended timed push body, batching/setup state materially changes the observed
  benchmark. Historical allocator-unlabelled numbers require revalidation.
- Remaining confounder hypothesis: child-visible runtime and Criterion paths still
  encoded `baseline` versus `candidate`, including unequal path lengths. This may
  alter process/heap layout before a sub-percent microbenchmark; it is not yet an
  established explanation.

### Label-neutral child-process calibration

- Status: accepted as the initial wall-time noise calibration
- Change from the second rejected calibration: use one runtime working directory
  for both labels and equal-length round/position Criterion paths. The child process
  receives no baseline/candidate label; mapping occurs only in parent-written
  metadata after execution.
- Calibration hypothesis: removing all known child-visible stable label encoding
  will eliminate the directional A/A result for identical commit `c868598` on
  `push_preallocated/ThinVec/1024`.
- Primary metric and success rule: unchanged—absolute median paired delta at most
  1%, deterministic bootstrap interval includes zero, byte-identical builds, and
  all seven declared rounds retained.
- Pre-registered parameters: seed `20260712`; all other commits, filter, CPU,
  allocator clearing, Criterion parameters, and fixed seven-round stopping rule are
  unchanged. Expected result: no effect; failure blocks A/B optimization claims and
  triggers a redesign of the timing method rather than another post-hoc rerun.
- Result: passed. The median paired A/A delta was +0.779%, the paired range was
  [-0.735%, +0.889%], and the deterministic bootstrap interval included zero at
  [-0.591%, +0.860%]. All 14 measurements completed with identical binary hash,
  device, inode, executable path, runtime directory, and equal-length artifact paths.
- Interpretation: for this host, allocator, and 1,024-element push workload, effects
  below 1% are inside the observed process-level envelope and are inconclusive.
  This is an initial workload-specific calibration, not a universal noise constant.

### Retrospective push revalidation

- Status: wall-time, allocation, codegen, code-size, and correctness gates passed;
  fixed-work counters remain an optional upstream-strengthening gate
- Baseline commit: `f8fa1e8` (corrected benchmark boundary, old push)
- Candidate commit: `c3b80a1` (outlined growth and known-length publication)
- Hypothesis: removing repeated header length/capacity traffic from no-growth push
  materially reduces the time to initialize 1,024 preallocated `u64` elements.
- Primary metric: paired wall-time delta for
  `push_preallocated/ThinVec/1024` under explicitly cleared preload/System malloc.
- Primary acceptance threshold: at least 25% faster at the median, bootstrap interval
  entirely below zero, and improvement far outside the calibrated 1% A/A envelope.
- Declared secondary variants: ThinVec lengths 1 and 4 from the same
  `push_preallocated/ThinVec` filter. Report both; neither may regress beyond the 1%
  calibrated envelope. Do not select a favorable size after measurement.
- Fixed parameters: seven paired rounds, seed `20260713`, CPU 0, sample size 100,
  3-second warm-up, 5-second measurement, 100,000 resamples, preload cleared, and
  label-neutral child paths. Do not extend or remove rounds.
- Secondary evidence required after wall time: executable/code-size comparison,
  optimized disassembly supporting the proposed load/store mechanism, fixed-work
  instructions and cycles, unchanged allocation metrics, and existing correctness
  gates. Wall time alone cannot complete retrospective acceptance.
- Expected result: a large improvement consistent with the historical result. A
  materially smaller result is a reason to revisit allocator and benchmark-state
  sensitivity, not to preserve the old claim.
- Wall-time result: passed all declared variants under cleared-preload/System malloc.
  One element improved 60.39%, four elements 73.85%, and 1,024 elements 78.68% at
  the paired medians. Every paired round was strongly favorable and every bootstrap
  interval was entirely below zero.
- Primary detail: 1,024 elements improved from a median per-process mean estimate of
  2,056.9 ns to 438.3 ns; its paired range was [-78.88%, -78.61%] and bootstrap
  interval [-78.76%, -78.64%]. This is far outside the calibrated A/A envelope.
- Code-size result: the complete benchmark executable decreased by 48 bytes
  (3,759,184 to 3,759,136). Retaining future built executables in the artifact set
  is now mandatory so hot-function disassembly remains auditable after worktree
  cleanup.
- Wall time alone did not complete revalidation; the allocation, disassembly,
  code-size, and correctness results below supplied the declared independent checks.
- Allocation result: passed. Running the deterministic allocation executable on the
  exact parent and candidate with preload cleared produced identical CSV rows for
  every workload, including requested/peak bytes, allocation/reallocation/drop
  counts, and zero live bytes after destruction.
- Disassembly result: supports the mechanism. The old 1,024-element hot loop calls a
  large non-inlined `BenchVector::push` wrapper for every element; that wrapper
  contains register-save overhead and inlined growth machinery. The candidate loop
  contains the expected capacity comparison, value store, length increment/store,
  and loop branch, with growth behind a cold call.
- Correctness result: the exact candidate commit already passed the native, no_std,
  Gecko, Rust 1.86, Clippy, and strict-provenance Miri gates recorded in the original
  push experiment. No new implementation code is under test here.
- Remaining decision: fixed-work instruction/cycle calibration and comparison. Do
  not grow the permanent benchmark suite merely to obtain it; use a small temporary
  driver only if required before upstream submission. The implementation result is
  not otherwise awaiting another timing experiment.

### Retrospective append revalidation

- Status: timing and focused-codegen gates passed; small total text tradeoff recorded
- Baseline commit: `55ca926` (append benchmark present, iterator-driven append)
- Candidate commit: `e6fbaaf` (single reserve plus bulk relocation)
- Hypothesis: replacing drain/extend with one non-overlapping bulk copy materially
  reduces the cost of moving 1,024 preallocated `u64` elements without worsening
  the small four-element case.
- Primary metric: paired wall-time delta for
  `append_preallocated/ThinVec/1024` under explicitly cleared preload/System malloc.
- Primary threshold: at least 30% faster at the median, bootstrap interval entirely
  below zero, and improvement far outside the calibrated 1% A/A envelope.
- Declared secondary variant: `append_preallocated/ThinVec/4`; report every round
  and reject a regression beyond the calibrated 1% envelope.
- Fixed parameters: filter `append_preallocated/ThinVec`, seven paired rounds, seed
  `20260714`, CPU 0, sample size 100, 3-second warm-up, 5-second measurement,
  100,000 resamples, preload cleared, and label-neutral child paths. Do not extend
  or remove rounds.
- Secondary evidence after a timing pass: executable size and focused optimized
  disassembly. Existing ownership/ZST and strict-provenance tests remain the
  correctness gate; do not add another permanent benchmark or profiling framework.
- Expected result: a large 1,024-element improvement and a smaller positive
  four-element result, consistent with the original experiment.
- Wall-time result: passed. Four elements improved 21.18% at the paired median with
  bootstrap interval [-22.25%, -20.60%]. The primary 1,024-element case improved
  68.89%, from 506.8 ns to 157.8 ns, with interval [-69.07%, -68.53%]. Every
  declared paired round favored the candidate by a wide margin.
- Focused codegen result: supports the mechanism. The relevant Criterion
  monomorphization shrank from 0x64d to 0x58f bytes (190 bytes) and replaces the
  baseline iterator/fold/push calls with direct reserve handling and `memcpy`.
- Whole-binary size tradeoff: the ELF `.text` section grew 1,140 bytes, or about
  0.038% (3,010,112 to 3,011,252), and data grew 24 bytes. File size grew 4,416
  bytes including layout/padding. This does not come from growth of the measured hot
  monomorphization, but its remaining cross-symbol/link-layout attribution is not
  proven; retain it as an explicit cost rather than calling code size unchanged.
- Correctness evidence remains the dedicated owning-element/ZST tests and native plus
  Gecko strict-provenance Miri gates recorded for the exact implementation commit.
- Tooling note: `cargo-bloat` 0.12.1 is installed on the builder, but it cannot
  directly consume an already-built Cargo bench executable. Do not add a synthetic
  Cargo project solely to hide or over-explain this small recorded delta.

### Post-append implementation audit

- Status: research complete; splice finding implemented, later candidates pending
- Highest-confidence finding: `Drain::move_tail`, used by `splice`, passes
  `end + tail + additional` to `ThinVec::reserve`. `reserve` interprets its
  argument as elements additional to the vector's current initialized prefix,
  so the prefix is counted twice and splice can grow substantially beyond the
  required capacity. Rust's `Vec` passes the initialized layout length and the
  true additional count as separate arguments to `RawVec::reserve`; ThinVec
  cannot copy that expression directly through its public reserve API.
- Outcome: corrected and validated on `perf/splice-reserve`; retain this paragraph as
  the audit trail that led to the change, not as an outstanding task.
- Next generalized candidates: guarded hole/backshift algorithms for
  `retain_mut` and `dedup_by`, one-shot length publication for bulk extension,
  and slice-tail destruction in `truncate`.
- Measurement finding: Criterion's time-based profiling mode performs a
  different number of operations for implementations with different throughput.
  Raw `perf stat` totals from those runs are therefore not comparable. Hardware
  counter comparisons require a fixed-work driver or per-operation normalization.
- Profiling tools: Linux `perf`, Samply, and Valgrind are available on the remote
  host. Use `perf stat` for deterministic fixed-work counters and Samply only
  when attribution is needed; neither should add benchmarks without a concrete
  decision to distinguish.

### Bulk append (`perf/bulk-append`)

- Status: accepted
- Hypothesis: one reserve and one bulk relocation will outperform the current
  `extend(other.drain(..))` element loop without changing allocation behavior.
- Affected measurement: `append_preallocated` at 4 and 1,024 elements.
- Acceptance: reproducible CPU improvement with an explicit, acceptable code-size
  tradeoff, identical final contents, source clearing, allocation behavior, panic
  safety, and Gecko auto-array ownership.
- Baseline: ThinVec took 4.364 ns for 4 elements and 543.7 ns for 1,024 elements;
  Vec took 2.459 ns and 194.4 ns.
- Result: accepted. ThinVec improved to 3.472 ns for 4 elements (about 20%) and
  211.5 ns for 1,024 elements (about 61%). Large append is now close to Vec's
  196.8 ns rather than nearly three times slower.
- Safety result: dedicated owning-element and ZST tests prove source clearing and
  exactly-once destruction; native and Gecko strict-provenance Miri tests pass.
- Retrospective size result: the measured append hot monomorphization shrank 190
  bytes, while total ELF `.text` grew 1,140 bytes (0.038%). Treat that whole-binary
  increase as a recorded cost, not a code-size improvement.

### Push fast path (`perf/push-fast-path`)

- Status: accepted
- Hypothesis: publishing the known length directly and outlining growth will reduce
  redundant header traffic, register pressure, and hot-path code size.
- Affected measurements: `push_preallocated` and `build_growing_and_drop` only.
- Baseline: preallocated ThinVec push is 1.98x slower at 1 element, 3.56x at
  4 elements, and 4.36x at 1,024 elements on the pinned Linux host.
- Acceptance: reproducible improvement without memory, traversal, correctness, or
  code-size regression; otherwise revert the experiment.
- Result: accepted. Preallocated ThinVec push improved from 2.448 ns to 0.986 ns
  at 1 element, 8.645 ns to 2.163 ns at 4 elements, and 2.113 us to 0.476 us at
  1,024 elements. The 1- and 4-element cases beat Vec; 1,024 elements reached
  near-parity.
- Full lifecycle result: building, growing, and dropping 1,024 elements takes
  0.562 us for ThinVec versus 0.668 us for Vec on the pinned Linux host.
- Memory result: requested bytes, allocation counts, reallocation counts, and zero
  live bytes after drop are unchanged.
- Code-size result: the benchmark executable's text section decreased by 220 bytes
  (2,979,384 to 2,979,164 bytes).
- Safety result: all supported test lanes and both strict-provenance Miri
  configurations pass.
- Allocator note: the original figures inherited tcmalloc. Exact-parent System-malloc
  revalidation later reproduced 60-79% improvements across the declared sizes.

## Verified baseline

### Representation and requested memory

On the measured 64-bit targets:

| Property | `Vec<T>` | `ThinVec<T>` |
|---|---:|---:|
| Owner size | 24 B | 8 B |
| Native nonempty header | 0 B | 16 B |
| Empty allocation | none | none; shared singleton |

For 10,000 nested `u64` containers:

| Workload | `Vec` requested bytes | `ThinVec` requested bytes | Result |
|---|---:|---:|---:|
| Empty | 240,000 | 80,000 | ThinVec uses 67% less |
| Sparse | 304,000 | 176,000 | ThinVec uses 42% less |
| Four elements each | 560,000 | 560,000 | Equal |

Requested bytes are not allocator usable size or RSS. The allocation runner also
asserts that every workload returns to zero live requested bytes after drop.

### Historical remote CPU baseline

These measurements inherited the builder host's tcmalloc preload. Their internal A/B
comparisons remain hypotheses worth reproducing, but they are not glibc-System
results and are not final evidence under the current protocol.

Ten thousand nested vectors:

| Workload | `Vec` | `ThinVec` | Result |
|---|---:|---:|---:|
| Construct/drop empty | 14.241 us | 10.515 us | ThinVec 26% faster |
| Construct/drop sparse | 35.468 us | 34.295 us | ThinVec 3% faster |
| Construct/drop four elements | 111.85 us | 172.90 us | ThinVec 55% slower |
| Traverse empty | 2.196 us | 2.152 us | ThinVec 2% faster |
| Traverse sparse | 3.289 us | 3.451 us | ThinVec 5% slower |
| Traverse four elements | 21.667 us | 21.533 us | Effectively equal |

Sequential iteration is effectively identical:

| Elements | `Vec` | `ThinVec` |
|---:|---:|---:|
| 8 | 1.606 ns | 1.605 ns |
| 1,024 | 101.49 ns | 101.59 ns |

Corrected preallocated push-only measurements exclude allocation and destruction:

| Elements pushed | `Vec` | `ThinVec` | ThinVec ratio |
|---:|---:|---:|---:|
| 1 | 1.234 ns | 2.448 ns | 1.98x |
| 4 | 2.427 ns | 8.645 ns | 3.56x |
| 1,024 | 484.7 ns | 2.113 us | 4.36x |

### Established interpretation

- ThinVec's final memory density is valuable and measurable.
- Sequential reading is already at parity; do not optimize it without contrary
  evidence.
- The primary generalized CPU problem is mutation/construction, not traversal.
- Equal allocation counts do not explain the push gap.
- Current ThinVec places slow allocation/layout machinery and allocation-resident
  header state in the construction path.
- Platform ratios differ because CPU code generation, allocator size classes,
  realloc behavior, and cache topology differ. Compare each platform against its
  own historical baseline.

## Experimental protocol

Performance work is vulnerable to confirmation bias, benchmark-boundary mistakes,
and accidental changes in compiler output. An attractive result is a reason to
investigate more carefully, not a reason to lower the evidence bar.

### Pre-register each experiment

Before changing implementation code, record:

- the exact hypothesis and proposed mechanism;
- one primary metric and the smallest workload that can falsify the hypothesis;
- secondary metrics needed to detect displaced cost: allocation, destruction,
  code size, retained capacity, or downstream traversal;
- the exact baseline commit, experiment commit, toolchain, target features,
  allocator, benchmark command, and environment;
- expected success, no-effect, and regression outcomes;
- an acceptance threshold chosen from the measured noise floor and practical
  importance, not after seeing the result;
- minimum and maximum independent round counts plus a stopping rule, fixed before
  measurement so a run cannot stop merely when the desired answer appears;
- every input size and variant that will be reported, including unfavorable ones.

Change one mechanism per branch. Do not combine individually unmeasured changes,
and do not tune the benchmark after inspecting a favorable implementation result.

### Establish a trustworthy A/B comparison

- Build the exact parent commit and experiment commit in separate clean worktrees.
- Keep measurement-only changes in a shared harness commit or external driver and
  apply them identically to both implementation worktrees.
- Use the same locked dependencies, rustc, target, profile, `RUSTFLAGS`, allocator,
  LTO/codegen-unit settings, CPU affinity, and benchmark source for both sides.
- Never use the experiment branch's historical prose as the baseline measurement.
- Alternate or randomize A/B execution order across at least seven independent
  process-level rounds, continuing to the pre-registered maximum if stability
  criteria are not met. Criterion samples inside one process are not independent
  experimental repetitions.
- Pin the Linux run to the same physical CPU, record governor and frequency state,
  reject runs with migrations or material background activity, and allow equivalent
  warm-up before each measurement.
- Retain raw per-round artifacts. Report the paired deltas, median, spread, and
  confidence interval; do not report only the best run or only Criterion's final
  point estimate.
- Calibrate the environment's same-binary repeatability before interpreting a
  small effect. A result inside that noise floor is inconclusive, not a win.

### Audit the benchmark boundary

- State whether setup, allocation, growth, element initialization, destruction,
  and deallocation are inside or outside the timed region and why.
- Validate contents and ownership outside the timed region. Make both inputs and
  observable outputs opaque enough to prevent constant folding or dead-code
  elimination.
- Ensure A and B perform the same semantic work. Capacity, iterator behavior,
  clone/drop cost, allocation state, and final contents must not differ unless that
  difference is the declared subject of the experiment.
- Measure an isolated operation and its full lifecycle separately when moving cost
  across the timing boundary is possible.
- Inspect optimized assembly or disassembly for sub-nanosecond results, surprising
  speedups, and changes intended to affect loads, branches, or inlining.
- Prefer a temporary focused harness while investigating. Keep a benchmark in the
  permanent suite only if it protects a distinct decision or regression.

### Triangulate rather than trust one metric

- Wall time is the outcome metric; deterministic fixed-work instructions, cycles,
  branches, and cache events are explanatory evidence.
- Hardware-counter comparisons must execute the same fixed operation count or be
  normalized by a verified count. Time-based Criterion profile totals are invalid
  for direct A/B counter comparison because throughput changes iteration count.
- If wall time improves while instructions, memory traffic, or the proposed
  mechanism do not, treat the result as unexplained and investigate before accepting.
- Track total and hot-function code size. A local speedup caused by aggressive
  inlining may regress instruction cache and downstream binaries.
- For memory claims, distinguish requested bytes, allocator usable bytes, allocation
  and reallocation counts, peak live bytes, retained capacity, and RSS. Never use
  one as a synonym for another.
- Keep global-allocation instrumentation out of CPU timing runs; its bookkeeping can
  perturb allocator and synchronization behavior.
- Record whether reallocation stayed in place or moved. Allocator luck can otherwise
  masquerade as a container improvement.
- Confirm generalized claims on a second code-generation/platform context or label
  them platform-specific. Keep macOS and Linux numerical baselines separate.

### Try to disprove correctness and performance claims

- Exercise empty, singleton, growth-boundary, large, ZST, over-aligned, owning-drop,
  and panicking clone/drop/predicate cases as relevant.
- Test iterators with exact, underestimated, overestimated, and adversarial size
  hints. Safe iterator metadata is never an unsafe initialization guarantee.
- Run native, `no_std`, MSRV, Clippy, documentation, and strict-provenance
  Miri lanes appropriate to the changed code.
- Check final length, capacity, allocation layout, source ownership, exactly-once
  destruction, and unwind state explicitly; output equality alone is insufficient.
- Run the sqlparsers end-to-end workload before calling a microbenchmark win useful
  for the motivating AST use case. Check construction, traversal/rendering,
  allocations, retained memory, and pinned node sizes.
- Seek counterexamples and regressions first. Record null and negative results in
  the decision log with the same fidelity as accepted work.

### Decision rule

Accept a change only when the pre-registered primary metric clears its threshold in
repeatable paired runs, the mechanism is supported by independent evidence, and no
required correctness, memory, code-size, lifecycle, or downstream gate regresses.
An unexplained result, a mixed result hidden by averaging, or a result obtained only
after selecting favorable sizes is inconclusive. Revert inconclusive experiments;
preserve their evidence here.

## P0: measurement integrity

- [x] Add Criterion CPU benchmarks comparing `Vec` and `ThinVec`.
- [x] Add a separate allocation-counting runner with CSV output.
- [x] Separate preallocated push timing from allocation and destruction.
- [x] Reduce operation sizes to high-signal points: 1, 4, and 1,024.
- [x] Pin remote CPU measurements to one physical core/cache domain.
- [ ] Add a metadata-only nested scan: `len`, `is_empty`, and possibly `capacity`.
- [ ] Add exact growth-transition cases only where needed by a growth experiment.
- [ ] Add requested bytes copied and pointer-stayed versus pointer-moved reallocs.
- [x] Add allocator usable-byte diagnostics on macOS and glibc Linux.
- [x] Record rustc commit, target, CPU, OS, allocator, and governor with retained
  benchmark artifacts automatically.
- [x] Replace mutable saved-`main` comparisons with exact-commit detached worktrees
  using an identical benchmark tree and lockfile.
- [x] Add a reproducible A/B runner that builds explicit commits in separate
  worktrees, alternates their order, and retains raw per-round artifacts.
- [x] Measure same-binary timing repeatability for the pinned Linux 1,024-element
  push lane after eliminating stable child-visible labels.
- [ ] Establish same-binary noise for the fixed-work counter lane.
- [x] Retrospectively revalidate push wall time, allocation behavior, codegen, and
  code size against its exact parent under the experimental protocol.
- [x] Retrospectively revalidate append wall time and focused codegen against its
  exact parent, recording the whole-binary text tradeoff.
- [ ] Treat the smallest push results as provisional superiority claims until
  disassembly and independent paired rounds rule out sub-nanosecond artifacts.
- [ ] Re-establish allocator-labelled baselines with preload explicitly controlled;
  the builder host injects tcmalloc globally, so prior “glibc” labels are invalid.

Keep new benchmarks scarce. A benchmark must distinguish a concrete design choice
or protect an established property. Remove redundant sizes and methods.

## P1: generalized JackVec wins

Each change below should be isolated, measured, and either retained or reverted
before combining it with another optimization.

### Push fast path

- [ ] Add a focused assembly wrapper for no-growth push on x86-64 and AArch64.
- [x] Combine length and capacity retrieval where both are required.
- [x] Remove redundant header reads between `push` and `push_unchecked`.
- [x] Outline `grow_one` into a cold, non-inlined slow path.
- [x] Confirm the common path contains only state load, capacity branch, element
  write, and length publication.
- [x] Measure paired wall time, hot-function bytes, and total code size.
- [ ] Measure fixed-work instructions and cycles only if required for upstream
  submission; do not add a permanent benchmark solely for this purpose.

### Bulk operations

- [x] Replace `append(other.drain(..))` with reserve plus bulk relocation.
- [x] Add one focused append benchmark with small and large source lengths.
- [x] Correct `Drain::move_tail` reserve accounting so splice reserves only the
  capacity required for the preserved prefix, moved tail, and replacement.
- [x] Add a focused splice capacity regression test that demonstrates the current
  prefix double-count without expanding the benchmark suite.
- [x] Replace swap-based `retain_mut` with a guarded hole/backshift algorithm.
- [x] Benchmark retain only if implementing it: mixed rejection and large `T` are
  the high-signal cases.
- [x] Replace swap-based `dedup_by` with a guarded first-hole/backshift algorithm;
  avoid moving duplicate values into the retained prefix merely to drop them later.
- [x] Test setting final length once and bulk-dropping the tail in `truncate`; reject
  it after a 0.65% result, code growth, and observable drop-order change.
- [x] Verify bulk-truncate panic behavior with destructors that panic before
  rejecting and reverting the candidate.

### Bulk construction and extension

- [x] Keep a local initialized length while consuming the reserved lower-bound
  portion of `Extend`, publishing header length through a panic guard instead of
  loading and storing it for every element.
- [x] Give the compile-time exact array conversion a direct relocation path; keep
  safe `ExactSizeIterator` as a reservation hint, never an unsafe trust boundary.
- [x] Make `resize` use its already-reserved unchecked construction path instead
  of repeating the public push capacity branch for every new element.
- [x] Inspect `extend_from_slice` before specializing: after guarded `Extend`, the
  AArch64 `u64` monomorph already vectorizes 64-byte chunks, carries length locally,
  and publishes it once. Do not add a redundant specialization or benchmark unless
  a nontrivial `Clone` workload later demonstrates adapter/fallback overhead.

### Clone and conversions

- [x] Add a partial-initialization guard to nonempty cloning.
- [x] Treat the current clone panic leak as a quality/correctness issue: cloned
  elements written before a later `T::clone` panic are not represented by `len`
  and therefore are not dropped.
- [x] Reevaluate `cold`/`inline(never)` on the nonempty clone path; normal inlining
  wins at four elements but loses at 1,024, so retain the outlined policy.
- [x] Benchmark small and moderately large nonempty clones, then remove the rejected
  policy benchmark rather than permanently expanding the suite.
- [x] Inspect whether `Vec`/`ThinVec`/boxed-slice conversions already become bulk
  copies before rewriting them. `Vec`/boxed-slice into ThinVec vectorizes its bulk
  move, but ThinVec into Vec uses generic per-element collection and ThinVec into
  boxed slice inherits it before a possible exact-capacity shrink.

### Removed compatibility lane

- [x] Remove Gecko/nsTArray representation and growth behavior.
- [x] Remove AutoThinVec and its stack-buffer ownership model.
- [x] Remove Gecko-only tests, documentation, feature flags, and CI lanes.

## P1: spiritual-successor prototype

Use a separate experimental type until representation, safety, and performance are
proven. Do not silently alter ThinVec's stable native layout.

### Compact final representation

- [ ] Prototype a one-word `LeanVec<T>` owner.
- [ ] Prototype an 8-byte common header: `{ len: u32, cap: u32 }`.
- [ ] Evaluate an explicitly compact-only type versus a tagged rare wide-header
  fallback for capacities beyond `u32::MAX`.
- [ ] Preserve the singleton, slice surface, Option niche, Send/Sync behavior, and
  exact deallocation layout.
- [ ] Measure requested and usable bytes across `u8`, `u64`, AST-sized, and
  over-aligned elements around allocator size-class boundaries.

### Transient builder

- [ ] Prototype `LeanVecBuilder<T>` with pointer, length, and capacity inline.
- [ ] Keep builder length/capacity in registers during repeated pushes.
- [ ] Allocate the final prefix header from the start so finalization can transfer
  ownership without moving elements.
- [ ] Publish final header state once in `finish()`.
- [ ] Ensure builder Drop handles partially initialized elements and parse errors.
- [ ] Preserve mutation on the final `LeanVec`; the builder optimizes the common
  construction phase without freezing the public collection.
- [ ] Compare builder push against both current ThinVec and Vec at 1, 4, and 1,024.

### Exact construction

- [ ] Add guarded `one`, `two`, `from_array`, and `from_fn` constructors.
- [ ] Centralize partial-initialization panic guards using `MaybeUninit`.
- [ ] Never trust safe `ExactSizeIterator` as an unsafe initialization guarantee.
- [ ] Evaluate fixed-arity one-word `HeapArray<T, N>` only for true arity invariants.

## P2: situational successor experiments

These are evidence-gated and should not complicate the baseline type until a real
workload demonstrates a win.

### Builder-only inline scratch

- [ ] Collect real final-length histograms before choosing an inline count.
- [ ] Prototype builder scratch counts 2 and 4; do not put inline elements in the
  final collection.
- [ ] Measure stack size, spill rate, copy cost, final retained bytes, and full parse
  time for AST-sized elements.

### Pointer-tagged cached length

- [ ] Prototype low-bit caching for empty/singleton/small lengths only after the
  builder and compact header land.
- [ ] Measure metadata-only traversal and randomized nested access.
- [ ] Validate pointer masking, deallocation, Option niche, drains, panicking Drop,
  ZSTs, over-aligned elements, and cross-thread moves under strict-provenance Miri.

### Canonical capacity classes

- [ ] Compare a packed `len + capacity-class` header with direct `u32 len/cap`.
- [ ] Ensure the class reconstructs the exact requested allocation layout.
- [ ] Measure spare capacity, moved reallocations, usable bytes, and growth CPU
  across macOS malloc, glibc, and mimalloc.

### Caller-informed construction policy

- [ ] Keep policy in the builder or construction call, not the final public type.
- [ ] Evaluate exact, singleton-biased, small-list, and geometric growth policies
  against measured final-length distributions.
- [ ] Avoid per-vector stateful allocators and unchecked allocator usable-size
  capacity.

## sqlparsers proving workload

Verified characteristics:

- Roughly 310 `ThinVec<...>` type occurrences across approximately 130 element
  shapes.
- Production AST child lists use ThinVec rather than Vec.
- Deep cases include `ThinVec<ThinVec<ValuesItem>>` and nodes with several
  independently optional child lists.
- Parser construction contains roughly 217 `ThinVec::new()` and 130 push sites but
  only four explicit `with_capacity` uses.
- Lists are commonly built once and traversed/rendered repeatedly; occasional public
  mutation must remain supported.
- Existing profiles show `ThinVec::push` in the node-building cost.
- Final AST node sizes are generated and contractually pinned.
- Final inline/spill storage is unacceptable because it enlarges every empty node.
- A whole-tree arena was already measured and rejected: a fast allocator recovered
  most of the benefit, leaving about 3.5-4.6% unique gain with significant ownership
  and safe-clone complications.

Tasks:

- [ ] Add corpus instrumentation for per-field `(len, capacity)` histograms.
- [ ] Record empty, 1, 2, 3, 4, 5-8, and greater-than-8 buckets.
- [ ] Attribute allocations, reallocations, requested bytes, and retained capacity to
  child-list construction rather than aggregate parser totals.
- [ ] Record mutation-after-parse frequency for public visitor/rewriter workflows.
- [ ] Patch sqlparsers to use `LeanVecBuilder` internally while keeping final fields
  one word and mutation-capable.
- [ ] Run its existing wall-time, deterministic instruction, allocation, retained
  memory, node-size, render, serde, and Miri gates.
- [ ] Require no regression in traversal or final AST size before adoption.

## Platform diagnostics

### Linux

- [x] Add optional `malloc_usable_size` reporting as a diagnostic only.
- [ ] Use `perf stat` for cycles, instructions, branches/misses, cache misses, dTLB
  misses, faults, context switches, and migrations.
- [ ] Use `perf record` only to attribute demonstrated regressions or wins.
- [ ] Add a fixed-operation micro-driver before comparing `perf stat` counter
  totals; Criterion `--profile-time` totals are throughput-dependent.
- [ ] Compare glibc System allocation with mimalloc and, where relevant,
  mozjemalloc.

### macOS

- [x] Add optional `malloc_size` reporting and compare requested with usable bytes.
- [ ] Use `xctrace` Time Profiler and CPU Counters on filtered, already-built
  benchmark executables.
- [x] Record moved reallocations and size-class transitions.
- [ ] Keep macOS and Linux baselines separate.

Hardware counters and allocator instrumentation explain wall-clock results; they do
not replace clean wall-clock measurements.

## Acceptance criteria

The experimental protocol above is mandatory. An optimization is retained only
when it satisfies the relevant subset below:

- Reproducible wall-time improvement on a clean, pinned host.
- Pre-registered primary metric clears a noise-informed practical threshold in
  independent, alternating paired A/B rounds.
- Reduced or unchanged instructions on deterministic Linux measurements.
- Optimized code supports the proposed mechanism; surprising results are explained.
- No unexplained code-size or hot-function growth.
- Reduced or unchanged requested and usable memory for its target workload.
- No hidden displacement between isolated-operation and full-lifecycle cost.
- No traversal regression for final collections.
- One-word owner and Option niche preserved.
- All allocation layouts reconstruct exactly.
- Existing tests, feature combinations, formatting, Clippy, and documentation pass.
- Miri passes strict-provenance and panic/drop torture cases for unsafe changes.
- sqlparsers final AST size pins and semantic APIs remain valid for AST-focused work.
- Raw artifacts and null/negative results are retained and summarized.

Situational changes must name their target distribution and must not be described as
general wins.

## Rejected or deferred directions

- Final SmallVec-style inline storage: rejected for recursive empty-heavy owners.
- Mandatory arena ownership: rejected as the primary successor; measured marginal
  benefit does not justify the ownership/API cost for the proving workload.
- Allocator usable size as portable capacity: rejected by GlobalAlloc layout
  contracts.
- Per-owner stateful allocators: rejected because they enlarge the owner or header.
- ZST pointer-length encoding: deferred until a real ZST-heavy workload exists.
- Broad pointer tagging: deferred until simpler builder/header wins are exhausted.
- Sequential-iteration optimization: no current problem; measured at parity.
- A single hyper-generic type combining allocator, growth, inline, width, and FFI
  policies: rejected due audit complexity, code bloat, and unstable tradeoffs.

## Decision log

### 2026-07-10: establish CPU and allocation baselines

- Added separate Criterion CPU and deterministic allocation runners.
- Chose `u64` as the initial representation baseline.
- Kept CPU and allocation instrumentation in different executables.

### 2026-07-10: raise MSRV to Rust 1.86

- Allows Criterion 0.8.2 and modern implementation techniques.
- Added an all-target MSRV CI job.

### 2026-07-10: correct push benchmark boundary

- `push_preallocated` now uses reference-batched setup.
- Allocation and destruction are excluded from the timed routine.
- Reduced the size matrix to 1, 4, and 1,024.
- Renamed the full growth lifecycle benchmark to `build_growing_and_drop`.

### 2026-07-10: adopt phase-split successor hypothesis

- Preserve the one-word final owner.
- Investigate compact headers and Vec-like transient builder state.
- Keep final mutation available for public AST rewrites.
- Treat builder-only inline scratch and pointer tags as later situational experiments.

### 2026-07-10: accept the optimized push fast path

- Public push loads header state once and publishes the already-known length.
- Growth and layout work is outlined behind a cold, non-inlined helper.
- Preallocated push improved by 59-79% across the measured sizes.
- ThinVec reached or beat Vec for one and four pushes and approached parity at
  1,024 pushes.
- The full 1,024-element growth lifecycle became about 16% faster than Vec.
- Requested memory and allocator-call counts were unchanged.
- Total benchmark text size decreased by 220 bytes.

### 2026-07-10: accept bulk append

- Replaced iterator-driven drain/extend with one reserve and bulk relocation.
- Four-element append improved by about 20%.
- 1,024-element append improved by about 61% and reached near-Vec performance.
- Added owning-element and ZST coverage; source elements drop exactly once.
- Native, no_std, Gecko, Rust 1.86, Clippy, and focused strict-provenance Miri gates
  pass.

### 2026-07-10: re-audit post-append mutation paths

- Compared ThinVec's mutation algorithms directly with Rust 1.86 `Vec`/`RawVec`.
- Found splice tail growth double-counting the initialized prefix through the
  mismatched `reserve` API semantics; prioritize its correction before new
  performance experiments.
- Identified `retain_mut`, `dedup_by`, `truncate`, `Extend`, `resize`, and clone as
  the remaining generalized implementation-level opportunities.
- Confirmed sequential traversal and the new push/append paths no longer justify
  broad optimization work.
- Rejected raw time-based profiler counter totals as cross-implementation evidence;
  fixed work is required for counter comparison.

### 2026-07-10: adopt skeptical experimental protocol

- Require pre-registered hypotheses, metrics, thresholds, variants, and failure
  outcomes before implementation work begins.
- Require clean exact-commit worktrees and alternating independent A/B rounds rather
  than trusting one Criterion process or a mutable branch baseline.
- Added benchmark-boundary, compiler-elision, code-size, allocator, lifecycle, and
  downstream checks intended to reveal displaced or accidentally omitted work.
- Require fixed-work normalization for hardware counters and separate definitions
  for requested, usable, peak-live, retained-capacity, and RSS memory claims.
- At adoption time, existing push and append results were only promising and required
  exact-parent reproduction. That reproduction is recorded in later entries.

### 2026-07-10: calibrate the paired runner and revalidate push timing

- Rejected two A/A calibrations that produced directional differences for identical
  code; did not relax the pre-registered interval rule because their medians were
  small.
- Discovered a global tcmalloc `LD_PRELOAD`, invalidating prior “glibc allocator”
  labels and demonstrating large allocator-conditioned push timing differences.
- Removed label-specific executable, working-directory, and artifact-path state from
  benchmark children. The resulting A/A interval included zero with every paired
  delta inside roughly 0.9%.
- Revalidated the push optimization against its exact parent under System malloc:
  60-79% improvements across all declared sizes with no total code-size regression.
- Allocation, optimized-codegen, code-size, and correctness evidence subsequently
  passed. Fixed-work counters remain optional strengthening before upstream submission.

### 2026-07-10: revalidate append timing and record its size tradeoff

- Revalidated append against its exact parent under label-neutral System malloc.
- Four elements improved 21.18%; 1,024 elements improved 68.89%, with every paired
  interval entirely favorable.
- Confirmed direct reserve plus `memcpy` in optimized code and a 190-byte reduction
  in the measured hot monomorphization.
- Recorded rather than hid the 1,140-byte (0.038%) total ELF `.text` increase.

### 2026-07-10: correct splice reserve accounting

- Demonstrated prefix double-counting against the exact parent: capacity 18 rather
  than 14 natively and 30 rather than 14 in Gecko at the selected size-class boundary.
- Changed `Drain::move_tail` to reserve only the moved tail plus new replacement
  elements; `reserve` accounts for the initialized prefix itself.
- Preserved contents, length, checked overflow, ownership, and iterator semantics.
- Added one capacity regression test rather than a low-signal timing benchmark.
- Native, no-default-feature, Gecko library, and focused strict-provenance Miri gates
  pass.

### 2026-07-10: reject generalized clone inlining

- Small clone-and-drop improved 12.92%, but the declared 1,024-element secondary
  regressed 4.17% in every paired round, so the policy was rejected.
- Disassembly showed a small-only vectorization benefit and an inlined large copy
  loop straddling a 32-byte fetch boundary. A forced-alignment diagnostic collapsed
  the large regression to 0.01%, supporting the negative result's cause.
- Restored `cold`/`inline(never)` and removed the temporary benchmark.

### 2026-07-10: accept direct ThinVec-to-Vec relocation

- Replaced generic iterator collection with one allocation, one bulk relocation,
  explicit ownership publication, and source-layout deallocation.
- Improved conversion by 9.29% at four elements and 80.64% at 1,024, with every
  paired round favorable.
- Preserved requested bytes, allocation/deallocation counts, exact returned
  capacity, peak-live memory, and exact-once destruction.
- Focused code shrank 370 bytes and whole-ELF text shrank 580 bytes.

### 2026-07-10: accept direct ThinVec-to-boxed-slice relocation

- Replaced boxed-slice iterator collection with one exact allocation, one bulk
  relocation, initialization publication, and source-layout deallocation.
- Improved the 1,024-element conversion 78.36% with unchanged requested memory,
  peak live bytes, and allocator-call counts.
- Whole-ELF text shrank 1,276 bytes; retained one high-signal benchmark rather than
  repeating the Vec conversion's small-size matrix.

### 2026-07-10: accept direct Vec-to-ThinVec relocation

- Improved the 1,024-element conversion 25.22% even though LLVM had vectorized the
  old collector; tuned `memcpy` and removal of iterator/reserve/fallback state remain
  material at 8 KiB.
- Preserved the exact allocation lifecycle, requested bytes, peak live bytes,
  destination capacity, and ownership semantics.
- The focused wrapper shrank 453 bytes and whole-ELF text shrank 1,804 bytes.

### 2026-07-10: reject boxed-input delegation on code size

- Allocation-free delegation to the accepted Vec relocation improved CPU time by
  31%, but actual whole `.text` grew 1,856 bytes through global inlining changes.
- Explicitly outlining the Box conversion preserved a 32% win and shrank its local
  helper, yet whole `.text` still grew 1,728 bytes because LLVM changed unrelated
  benchmark orchestration and nested-workload outlining.
- Reverted the implementation and removed its temporary test/benchmark rather than
  trading generalized, compiler-sensitive code growth for a situational conversion.

### 2026-07-10: accept direct array construction

- Used compile-time array length and initialization to bypass generic iterator and
  reserve machinery without trusting runtime size hints.
- Improved four-element construction 19.21% with one unchanged exact allocation and
  exact-once ownership transfer.
- The focused wrapper shrank 25 bytes; retained one small exact-construction lane.

### 2026-07-10: correct Gecko slow-growth threshold units

- Confirmed Gecko compared an element count with an 8 MiB byte threshold even
  though it had already computed header-inclusive requested bytes.
- One byte above the threshold previously jumped from an 8 MiB to a 16 MiB size
  class; corrected slow growth requests 9 MiB, saving 7 MiB/43.75%.
- Added exact boundary tests and left native, ZST, overflow, first-allocation,
  AutoThinVec, and ABI behavior unchanged.
