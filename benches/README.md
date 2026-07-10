# Performance benchmarks

The suite compares `ThinVec<u64>` with `Vec<u64>` along the dimensions affected by
their different representations. It deliberately does not benchmark every method:
most higher-level operations delegate to slices and would not isolate the tradeoff
this crate makes.

## CPU benchmarks

Run the statistically sampled Criterion benchmarks with:

```sh
cargo bench --bench cpu
```

The suite measures:

- construction and destruction of many empty, sparse, and uniformly small vectors;
- traversal of those nested-vector workloads;
- complete build-and-drop with normal growth;
- pushing into capacity allocated outside the timed region, with destruction also
  excluded from timing; and
- appending into preallocated destination capacity at small and large sizes; and
- retaining alternating elements from scalar and 64-byte-element vectors, with
  setup and final destruction excluded from timing; and
- sequential iteration after construction.

The operation sizes are deliberately limited to 1, 4, and 1,024 elements: singleton
lists, the common small-list/initial-growth boundary, and a stress case that exposes
accumulated hot-loop overhead. Sequential iteration uses only 8 and 1,024 elements
to cover short and vectorized scans without redundant intermediate points.

The sparse workload is deterministic: 80% of inner vectors are empty, 15% contain
one element, and 5% contain four. The nested workloads use 10,000 vectors, which is
large enough for their different inline sizes to affect cache behavior without
making routine benchmark runs unwieldy.

Criterion can compare a branch with a saved baseline:

```sh
cargo bench --bench cpu -- --save-baseline main
cargo bench --bench cpu -- --baseline main
```

For implementation decisions, use the repository's paired A/B runner rather than
Criterion's mutable saved baselines:

```sh
python3 tools/bench_ab.py \
  --baseline <exact-commit> \
  --candidate <exact-commit> \
  --filter push_preallocated/ThinVec/1024 \
  --exact \
  --rounds 7 \
  --seed 20260710 \
  --clear-preload \
  --cpu 0
```

The runner requires identical `Cargo.toml` and `benches/` trees, generates one
shared lockfile, builds detached worktrees before measurement, alternates A/B order,
stages every selected binary at the same executable path, and retains the raw
Criterion data plus paired summaries under
`benchmark-results/`. Omit `--cpu` on non-Linux systems. Shorter timings and fewer
rounds are suitable only for testing the runner, not for accepting performance
changes.

`--clear-preload` explicitly removes inherited `LD_PRELOAD` and
`DYLD_INSERT_LIBRARIES` values from build and benchmark children while retaining
both the inherited and effective environments in the manifest. Use it for the
System-allocator baseline; omit it only for a deliberately preloaded allocator
experiment.

Run benchmarks on an otherwise idle machine with CPU frequency scaling and thermal
conditions held as consistently as practical. Compare each implementation with its
own historical result; the `ThinVec`/`Vec` ratio describes a tradeoff and is not by
itself a regression threshold.

## Allocation metrics

Run the deterministic allocation accounting separately:

```sh
cargo bench --bench allocations
```

It emits CSV with inline container size, allocation and reallocation counts, live
requested bytes, and peak requested bytes for the nested, normally growing, and
reserved-capacity workloads. These are the sizes requested through Rust's global
allocator, not allocator usable size or RSS. That distinction makes the results
portable and repeatable, but allocator rounding and process-level memory overhead
are intentionally outside their scope.

The allocation runner is separate because wrapping the allocator with counters can
perturb CPU timings. Redirect its standard output to retain a machine-readable
artifact:

```sh
cargo bench --bench allocations > allocation-metrics.csv
```
