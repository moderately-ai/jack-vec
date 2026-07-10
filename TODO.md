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
- Working branch: `benchmarks/ab-runner`
- Initial benchmark commit: `5e4845a`
- Refined timing-boundary commit: `f8fa1e8`
- Persistent benchmark checkout: `catalyzed-builder:~/thin-vec`
- Benchmark toolchain: Rust 1.86
- Benchmark CPU: Ryzen 9 7950X3D, pinned to CPU 0 on the 96 MiB L3 CCD
- Benchmark OS/allocator: Ubuntu, Linux 5.15, glibc 2.35

## Active experiment

### Paired A/B runner and same-binary calibration (`benchmarks/ab-runner`)

- Status: runner implemented; remote calibration not yet run
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

### Post-append implementation audit

- Status: research complete; no implementation started
- Highest-confidence finding: `Drain::move_tail`, used by `splice`, passes
  `end + tail + additional` to `ThinVec::reserve`. `reserve` interprets its
  argument as elements additional to the vector's current initialized prefix,
  so the prefix is counted twice and splice can grow substantially beyond the
  required capacity. Rust's `Vec` passes the initialized layout length and the
  true additional count as separate arguments to `RawVec::reserve`; ThinVec
  cannot copy that expression directly through its public reserve API.
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
- Acceptance: reproducible CPU and code-size improvement with identical final
  contents, source clearing, allocation behavior, panic safety, and Gecko auto-array
  ownership.
- Baseline: ThinVec took 4.364 ns for 4 elements and 543.7 ns for 1,024 elements;
  Vec took 2.459 ns and 194.4 ns.
- Result: accepted. ThinVec improved to 3.472 ns for 4 elements (about 20%) and
  211.5 ns for 1,024 elements (about 61%). Large append is now close to Vec's
  196.8 ns rather than nearly three times slower.
- Safety result: dedicated owning-element and ZST tests prove source clearing and
  exactly-once destruction; native and Gecko strict-provenance Miri tests pass.

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
- Run native, `no_std`, Gecko, MSRV, Clippy, documentation, and strict-provenance
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
- [ ] Add allocator usable-byte diagnostics on macOS and glibc Linux.
- [ ] Record rustc commit, target, CPU, OS, allocator, and governor with retained
  benchmark artifacts automatically.
- [ ] Correct baseline documentation so `main` is measured from a separate checkout
  before the feature branch is compared against it.
- [ ] Add a reproducible A/B runner that builds explicit commits in separate
  worktrees, alternates their order, and retains raw per-round artifacts.
- [ ] Measure same-binary repeatability and establish practical noise floors for
  the pinned Linux timing and fixed-work counter lanes.
- [ ] Retrospectively revalidate push and append against their exact parent commits
  under the experimental protocol before proposing either change upstream.
- [ ] Treat the smallest push results as provisional superiority claims until
  disassembly and independent paired rounds rule out sub-nanosecond artifacts.

Keep new benchmarks scarce. A benchmark must distinguish a concrete design choice
or protect an established property. Remove redundant sizes and methods.

## P1: generalized ThinVec wins

Each change below should be isolated, measured, and either retained or reverted
before combining it with another optimization.

### Push fast path

- [ ] Add a focused assembly wrapper for no-growth push on x86-64 and AArch64.
- [x] Combine length and capacity retrieval where both are required.
- [x] Remove redundant header reads between `push` and `push_unchecked`.
- [x] Outline `grow_one` into a cold, non-inlined slow path.
- [ ] Confirm the common path contains only state load, capacity branch, element
  write, and length publication.
- [ ] Measure wall time, instructions, cycles, hot-function bytes, and code size.

### Bulk operations

- [x] Replace `append(other.drain(..))` with reserve plus bulk relocation.
- [x] Add one focused append benchmark with small and large source lengths.
- [ ] Correct `Drain::move_tail` reserve accounting so splice reserves only the
  capacity required for the preserved prefix, moved tail, and replacement.
- [ ] Add a focused splice capacity regression test that demonstrates the current
  prefix double-count without expanding the benchmark suite.
- [ ] Replace swap-based `retain_mut` with a guarded hole/backshift algorithm.
- [ ] Benchmark retain only if implementing it: mixed rejection and large `T` are
  the high-signal cases.
- [ ] Replace swap-based `dedup_by` with a guarded first-hole/backshift algorithm;
  avoid moving duplicate values into the retained prefix merely to drop them later.
- [ ] Set final length once and bulk-drop the tail in `truncate`; `clear` already
  drops the full slice behind a length-reset guard.
- [ ] Verify panic behavior with destructors that panic.

### Bulk construction and extension

- [ ] Keep a local initialized length while consuming the reserved lower-bound
  portion of `Extend`, publishing header length through a panic guard instead of
  loading and storing it for every element.
- [ ] Give exact/trusted internal construction paths a guarded direct-write loop;
  safe `ExactSizeIterator` remains only a reservation hint, never an unsafe trust
  boundary.
- [ ] Make `resize` use its already-reserved unchecked construction path instead
  of repeating the public push capacity branch for every new element.
- [ ] Specialize `extend_from_slice` around a guarded clone loop before adding a
  benchmark; use small and moderately large slices only if implementation begins.

### Clone and conversions

- [ ] Add a partial-initialization guard to nonempty cloning.
- [ ] Treat the current clone panic leak as a quality/correctness issue: cloned
  elements written before a later `T::clone` panic are not represented by `len`
  and therefore are not dropped.
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
- [ ] Add a fixed-operation micro-driver before comparing `perf stat` counter
  totals; Criterion `--profile-time` totals are throughput-dependent.
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
- Gecko behavior remains exact when the Gecko representation is involved.
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
- Existing push and append results remain promising, but must be retrospectively
  reproduced under this protocol before an upstream proposal.
