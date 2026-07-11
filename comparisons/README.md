# JackVec comparisons

This non-published workspace crate compares five explicit vector configurations:

- standard-library `Vec`;
- this checkout's `JackVec`;
- upstream `ThinVec` 0.2.18;
- `SmallVec<[T; 4]>` 1.15.1 with its `union` representation;
- `SmallVec<[T; 8]>` with the same configuration.

The names `SmallVec4` and `SmallVec8` are always reported separately. Inline
storage is a workload-dependent tradeoff, not a generic SmallVec result.

## What is measured

The CPU suite covers nested construction, traversal and metadata access; growing
and reserved construction; iteration; append; retain; dedup; extend; and resize.
All implementations receive identical input values and timing boundaries.
JackVec-only conversions remain in the root regression suite and are deliberately
excluded from cross-library rankings.

The allocation executable reports collection-owner size, requested and
allocator-usable live/peak bytes, allocation and reallocation counts, reallocation
movement, inline capacity, and SmallVec spill count. Owner bytes and live heap
bytes are different views: nested outer allocations already contain their inner
collection owners and the two columns must not be added.

## Commands

Quick validation:

```console
cargo test -p jack-vec-comparisons
cargo check -p jack-vec-comparisons --all-targets
cargo bench -p jack-vec-comparisons --bench allocations
cargo bench -p jack-vec-comparisons --bench cpu
```

An authoritative physical-host run requires a clean commit and performs five
Latin-square registration rotations:

```console
uv run --project comparisons/tools --locked python comparisons/tools/run_matrix.py --toolchain 1.97.0 --output-name macos-aarch64
uv run --project comparisons/tools --locked python comparisons/tools/run_matrix.py --toolchain 1.97.0 --cpu 0 --output-name linux-x86_64
uv run --project comparisons/tools --locked python comparisons/tools/validate_pair.py \
  comparisons/benchmark-results/macos-aarch64.json \
  comparisons/benchmark-results/linux-x86_64.json
```

The exact toolchain argument is mandatory and must match across hosts. The Linux
host is expected to pin CPU 0 on the Ryzen 7950X3D's 96 MiB V-cache
CCD, leave sibling CPU 16 idle, use the performance governor during measurement,
and restore its prior governor afterwards. The runner records but does not mutate
power-management policy. macOS runs should be on AC power with Low Power Mode
disabled and without other sustained work.

The runner resolves and verifies the requested `rustc` release, records its
commit and LLVM identity, waits for two consecutive idle samples after building
and between every round, records load and busy processes, and rejects a round if
sustained system load indicates contamination. `validate_pair.py` independently
refuses reports whose commit, compiler identity, schema, workload matrix, or
authoritative status differs.

Compact JSON and Markdown summaries in `benchmark-results/` are versioned. Raw
per-round estimates and allocation CSV files live under the ignored
`benchmark-results/raw/` directory.

## Interpretation

Every row includes absolute time and a paired ratio to `Vec`. A result is a win
only when its complete 95% paired bootstrap interval is below 0.97x, a loss only
when it is above 1.03x, equivalent only when the interval is wholly inside that
band, and inconclusive otherwise. Platforms are never pooled. Allocation results
are reported by individual metric rather than collapsed into a single winner.

CodSpeed CPU simulation runs on every pull request and `main` update. Managed
ARM64 Linux wall time runs on `main` and manual dispatch after the organization
enables public-repository macro-runner access and sets the repository variable
`CODSPEED_MACRO_ENABLED=true`. These continuous trends complement rather than
replace the controlled M4 macOS and x86_64 Linux reports.

## Python tooling

Reporting and visualization scripts are a non-packaged uv project rooted at
`comparisons/tools`. Python 3.11.13, uv 0.11.28 in CI, Matplotlib 3.11.0, and
all transitive dependencies are locked. Use `uv run --project
comparisons/tools --locked ...` from the repository root; do not install plotting
dependencies into the benchmark crate or the system Python environment.
