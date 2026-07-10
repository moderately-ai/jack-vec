# ThinVec performance roadmap

This document is the active project ledger for performance work on the
`tomsanbear/thin-vec` fork. Update it whenever an experiment starts, finishes,
changes priority, or produces a reusable finding. Record rejected ideas and the
evidence behind them so they are not repeatedly rediscovered.

## Objective

Push the one-word vector design toward state-of-the-art CPU and memory behavior,
with deeply nested, empty-heavy owned ASTs as the proving workload.

The central hypothesis is:

> ThinVec chose the right final representation, but uses that final
> representation during construction too. A successor can use Vec-like transient
> state while building and still produce a one-word final collection.

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
- Gecko/nsTArray ABI compatibility remains a separate, exact representation lane.
- Final inline element storage is out of scope because it destroys empty-container
  density in recursive structures.

## Repository state

- Fork: `https://github.com/tomsanbear/thin-vec`
- Working branch: `benchmarks/performance-suite`
- Initial benchmark commit: `5e4845a`
- Refined timing-boundary commit: `f8fa1e8`
- Persistent benchmark checkout: `catalyzed-builder:~/thin-vec`
- Benchmark toolchain: Rust 1.86
- Benchmark CPU: Ryzen 9 7950X3D, pinned to CPU 0 on the 96 MiB L3 CCD
- Benchmark OS/allocator: Ubuntu, Linux 5.15, glibc 2.35

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

### Remote CPU baseline

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

## P0: measurement integrity

- [x] Add Criterion CPU benchmarks comparing `Vec` and `ThinVec`.
- [x] Add a separate allocation-counting runner with CSV output.
- [x] Separate preallocated push timing from allocation and destruction.
- [x] Reduce operation sizes to high-signal points: 1, 4, and 1,024.
- [x] Pin remote CPU measurements to one physical core/cache domain.
- [ ] Add a metadata-only nested scan: `len`, `is_empty`, and possibly `capacity`.
- [ ] Add exact growth-transition cases only where needed by a growth experiment.
- [ ] Add requested bytes copied and pointer-stayed versus pointer-moved reallocs.
- [ ] Add allocator usable-byte diagnostics on macOS and glibc Linux.
- [ ] Record rustc commit, target, CPU, OS, allocator, and governor with retained
  benchmark artifacts automatically.
- [ ] Correct baseline documentation so `main` is measured from a separate checkout
  before the feature branch is compared against it.

Keep new benchmarks scarce. A benchmark must distinguish a concrete design choice
or protect an established property. Remove redundant sizes and methods.

## P1: generalized ThinVec wins

Each change below should be isolated, measured, and either retained or reverted
before combining it with another optimization.

### Push fast path

- [ ] Add a focused assembly wrapper for no-growth push on x86-64 and AArch64.
- [ ] Combine length and capacity retrieval where both are required.
- [ ] Remove redundant header reads between `push` and `push_unchecked`.
- [ ] Outline `grow_one` into a cold, non-inlined slow path.
- [ ] Confirm the common path contains only state load, capacity branch, element
  write, and length publication.
- [ ] Measure wall time, instructions, cycles, hot-function bytes, and code size.

### Bulk operations

- [ ] Replace `append(other.drain(..))` with reserve plus bulk relocation.
- [ ] Add one focused append benchmark with empty, small, and large source lengths.
- [ ] Replace swap-based `retain_mut` with a guarded hole/backshift algorithm.
- [ ] Benchmark retain only if implementing it: mixed rejection and large `T` are
  the high-signal cases.
- [ ] Set final length once and bulk-drop the tail in `truncate`/`clear`.
- [ ] Verify panic behavior with destructors that panic.

### Clone and conversions

- [ ] Add a partial-initialization guard to nonempty cloning.
- [ ] Reevaluate `cold`/`inline(never)` on the nonempty clone path.
- [ ] Benchmark empty, small, and moderately large nonempty clones.
- [ ] Inspect whether `Vec`/`ThinVec`/boxed-slice conversions already become bulk
  copies before rewriting them.

### Gecko correctness and performance

- [ ] Correct the slow-growth threshold to use total requested allocation bytes,
  matching current nsTArray semantics.
- [ ] Decide and document whether first allocation should be exact like nsTArray.
- [ ] Add requested-byte boundary tests around the 8 MiB slow-growth threshold.
- [ ] Build a linkable Gecko benchmark fixture, including AutoThinVec spill and
  return-to-inline behavior.

## P1: spiritual-successor prototype

Use a separate experimental type until representation, safety, and performance are
proven. Do not silently alter ThinVec's stable native or Gecko layouts.

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

- [ ] Add optional `malloc_usable_size` reporting as a diagnostic only.
- [ ] Use `perf stat` for cycles, instructions, branches/misses, cache misses, dTLB
  misses, faults, context switches, and migrations.
- [ ] Use `perf record` only to attribute demonstrated regressions or wins.
- [ ] Compare glibc System allocation with mimalloc and, where relevant,
  mozjemalloc.

### macOS

- [ ] Add optional `malloc_size` reporting and compare requested with usable bytes.
- [ ] Use `xctrace` Time Profiler and CPU Counters on filtered, already-built
  benchmark executables.
- [ ] Record moved reallocations and size-class transitions.
- [ ] Keep macOS and Linux baselines separate.

Hardware counters and allocator instrumentation explain wall-clock results; they do
not replace clean wall-clock measurements.

## Acceptance criteria

An optimization is retained only when it satisfies the relevant subset below:

- Reproducible wall-time improvement on a clean, pinned host.
- Reduced or unchanged instructions on deterministic Linux measurements.
- No unexplained code-size or hot-function growth.
- Reduced or unchanged requested and usable memory for its target workload.
- No traversal regression for final collections.
- One-word owner and Option niche preserved.
- All allocation layouts reconstruct exactly.
- Existing tests, feature combinations, formatting, Clippy, and documentation pass.
- Miri passes strict-provenance and panic/drop torture cases for unsafe changes.
- Gecko behavior remains exact when the Gecko representation is involved.
- sqlparsers final AST size pins and semantic APIs remain valid for AST-focused work.

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
