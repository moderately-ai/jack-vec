# JackVec comparison: macos-aarch64

- Commit: `2dc82e0656ba2c3997c4dc2dd26b905ce621bb2f`
- Rust: `rustc 1.97.0 (2d8144b78 2026-07-07)`
- Platform: `macOS-15.7.4-arm64-arm-64bit`
- Allocator policy: `system`; effective override environment: `{'LD_PRELOAD': None, 'DYLD_INSERT_LIBRARIES': None}`
Rounds: 5; practical-equivalence band: 0.97–1.03× Vec.

A win or loss requires the complete paired bootstrap interval to clear the practical-equivalence band. Results that cross a boundary are reported as inconclusive.

## CPU

| Benchmark | Implementation | Median ns | Ratio | 95% interval | Result |
|---|---:|---:|---:|---:|---|
| append_preallocated/1024 | Vec | 376.340 | 1.000× | 1.000–1.000× | baseline |
| append_preallocated/1024 | JackVec | 321.340 | 0.854× | 0.837–0.986× | inconclusive |
| append_preallocated/1024 | ThinVec | 887.060 | 2.412× | 2.248–2.780× | loss |
| append_preallocated/1024 | SmallVec4 | 261.562 | 0.721× | 0.580–0.798× | win |
| append_preallocated/1024 | SmallVec8 | 241.171 | 0.686× | 0.597–0.716× | win |
| append_preallocated/4 | Vec | 3.202 | 1.000× | 1.000–1.000× | baseline |
| append_preallocated/4 | JackVec | 4.239 | 1.327× | 1.288–1.355× | loss |
| append_preallocated/4 | ThinVec | 3.649 | 1.137× | 1.122–1.248× | loss |
| append_preallocated/4 | SmallVec4 | 4.477 | 1.395× | 1.380–1.452× | loss |
| append_preallocated/4 | SmallVec8 | 4.433 | 1.387× | 1.318–1.427× | loss |
| build_growing_and_drop/1 | Vec | 15.410 | 1.000× | 1.000–1.000× | baseline |
| build_growing_and_drop/1 | JackVec | 16.204 | 1.051× | 1.018–1.052× | inconclusive |
| build_growing_and_drop/1 | ThinVec | 15.950 | 1.035× | 1.033–1.049× | loss |
| build_growing_and_drop/1 | SmallVec4 | 2.550 | 0.165× | 0.162–0.186× | win |
| build_growing_and_drop/1 | SmallVec8 | 2.737 | 0.178× | 0.171–0.203× | win |
| build_growing_and_drop/1024 | Vec | 648.083 | 1.000× | 1.000–1.000× | baseline |
| build_growing_and_drop/1024 | JackVec | 720.598 | 1.109× | 1.005–1.194× | inconclusive |
| build_growing_and_drop/1024 | ThinVec | 724.489 | 1.126× | 1.051–1.183× | loss |
| build_growing_and_drop/1024 | SmallVec4 | 2113.067 | 3.249× | 1.347–3.470× | loss |
| build_growing_and_drop/1024 | SmallVec8 | 2102.541 | 3.267× | 1.356–3.418× | loss |
| build_growing_and_drop/4 | Vec | 18.059 | 1.000× | 1.000–1.000× | baseline |
| build_growing_and_drop/4 | JackVec | 17.532 | 0.971× | 0.970–1.086× | inconclusive |
| build_growing_and_drop/4 | ThinVec | 17.283 | 1.018× | 0.956–1.039× | inconclusive |
| build_growing_and_drop/4 | SmallVec4 | 17.327 | 0.968× | 0.954–1.053× | inconclusive |
| build_growing_and_drop/4 | SmallVec8 | 17.103 | 0.992× | 0.896–1.004× | inconclusive |
| dedup_adjacent_pairs/64_byte | Vec | 389.948 | 1.000× | 1.000–1.000× | baseline |
| dedup_adjacent_pairs/64_byte | JackVec | 411.982 | 1.053× | 1.048–1.076× | loss |
| dedup_adjacent_pairs/64_byte | ThinVec | 497.673 | 1.271× | 1.266–1.297× | loss |
| dedup_adjacent_pairs/64_byte | SmallVec4 | 519.180 | 1.326× | 1.312–1.349× | loss |
| dedup_adjacent_pairs/64_byte | SmallVec8 | 519.967 | 1.334× | 1.278–1.339× | loss |
| dedup_adjacent_pairs/u64 | Vec | 570.235 | 1.000× | 1.000–1.000× | baseline |
| dedup_adjacent_pairs/u64 | JackVec | 567.369 | 0.996× | 0.957–1.053× | inconclusive |
| dedup_adjacent_pairs/u64 | ThinVec | 374.058 | 0.650× | 0.633–0.670× | win |
| dedup_adjacent_pairs/u64 | SmallVec4 | 362.482 | 0.636× | 0.626–0.652× | win |
| dedup_adjacent_pairs/u64 | SmallVec8 | 368.752 | 0.638× | 0.624–0.657× | win |
| extend_reserved_1024 | Vec | 289.301 | 1.000× | 1.000–1.000× | baseline |
| extend_reserved_1024 | JackVec | 322.314 | 1.135× | 1.039–1.176× | loss |
| extend_reserved_1024 | ThinVec | 294.253 | 1.019× | 0.989–1.126× | inconclusive |
| extend_reserved_1024 | SmallVec4 | 285.241 | 0.967× | 0.940–1.007× | inconclusive |
| extend_reserved_1024 | SmallVec8 | 288.008 | 1.006× | 0.958–1.011× | inconclusive |
| nested_construct_and_drop/empty | Vec | 11617.427 | 1.000× | 1.000–1.000× | baseline |
| nested_construct_and_drop/empty | JackVec | 9314.572 | 0.805× | 0.796–0.807× | win |
| nested_construct_and_drop/empty | ThinVec | 9368.621 | 0.815× | 0.802–0.818× | win |
| nested_construct_and_drop/empty | SmallVec4 | 23067.211 | 2.009× | 1.897–2.036× | loss |
| nested_construct_and_drop/empty | SmallVec8 | 22798.722 | 1.967× | 1.952–2.053× | loss |
| nested_construct_and_drop/small | Vec | 196601.351 | 1.000× | 1.000–1.000× | baseline |
| nested_construct_and_drop/small | JackVec | 211247.930 | 1.061× | 1.012–1.097× | inconclusive |
| nested_construct_and_drop/small | ThinVec | 210396.828 | 1.076× | 1.050–1.084× | loss |
| nested_construct_and_drop/small | SmallVec4 | 182436.169 | 0.893× | 0.663–1.045× | inconclusive |
| nested_construct_and_drop/small | SmallVec8 | 150758.090 | 0.767× | 0.744–0.785× | win |
| nested_construct_and_drop/sparse | Vec | 48617.346 | 1.000× | 1.000–1.000× | baseline |
| nested_construct_and_drop/sparse | JackVec | 49504.636 | 1.014× | 0.998–1.038× | inconclusive |
| nested_construct_and_drop/sparse | ThinVec | 49137.618 | 1.002× | 0.991–1.046× | inconclusive |
| nested_construct_and_drop/sparse | SmallVec4 | 25679.187 | 0.528× | 0.523–0.578× | win |
| nested_construct_and_drop/sparse | SmallVec8 | 32746.079 | 0.667× | 0.643–0.729× | win |
| nested_metadata_scan_sparse | Vec | 1825.388 | 1.000× | 1.000–1.000× | baseline |
| nested_metadata_scan_sparse | JackVec | 3726.485 | 2.043× | 2.023–2.049× | loss |
| nested_metadata_scan_sparse | ThinVec | 3660.727 | 2.005× | 1.963–2.137× | loss |
| nested_metadata_scan_sparse | SmallVec4 | 3776.560 | 2.065× | 2.046–2.103× | loss |
| nested_metadata_scan_sparse | SmallVec8 | 5636.779 | 3.097× | 3.084–3.111× | loss |
| nested_traverse/empty | Vec | 2688.550 | 1.000× | 1.000–1.000× | baseline |
| nested_traverse/empty | JackVec | 2544.004 | 0.988× | 0.943–0.993× | inconclusive |
| nested_traverse/empty | ThinVec | 2562.218 | 0.962× | 0.953–0.991× | inconclusive |
| nested_traverse/empty | SmallVec4 | 3761.467 | 1.397× | 1.384–1.495× | loss |
| nested_traverse/empty | SmallVec8 | 5985.809 | 2.223× | 2.164–2.375× | loss |
| nested_traverse/small | Vec | 18038.168 | 1.000× | 1.000–1.000× | baseline |
| nested_traverse/small | JackVec | 18137.061 | 1.005× | 0.999–1.045× | inconclusive |
| nested_traverse/small | ThinVec | 18221.911 | 1.031× | 0.981–1.045× | inconclusive |
| nested_traverse/small | SmallVec4 | 18330.281 | 1.019× | 1.000–1.054× | inconclusive |
| nested_traverse/small | SmallVec8 | 18483.351 | 1.008× | 0.995–1.031× | inconclusive |
| nested_traverse/sparse | Vec | 5214.037 | 1.000× | 1.000–1.000× | baseline |
| nested_traverse/sparse | JackVec | 5225.069 | 0.983× | 0.960–1.061× | inconclusive |
| nested_traverse/sparse | ThinVec | 5214.489 | 1.002× | 0.981–1.019× | equivalent |
| nested_traverse/sparse | SmallVec4 | 5349.032 | 1.033× | 1.006–1.046× | inconclusive |
| nested_traverse/sparse | SmallVec8 | 5908.742 | 1.142× | 1.117–1.158× | loss |
| push_preallocated/1024 | Vec | 793.187 | 1.000× | 1.000–1.000× | baseline |
| push_preallocated/1024 | JackVec | 647.371 | 0.830× | 0.803–0.878× | win |
| push_preallocated/1024 | ThinVec | 676.931 | 0.865× | 0.853–0.896× | win |
| push_preallocated/1024 | SmallVec4 | 1843.217 | 2.319× | 2.081–2.515× | loss |
| push_preallocated/1024 | SmallVec8 | 1878.936 | 2.371× | 2.123–2.555× | loss |
| push_preallocated/4 | Vec | 2.716 | 1.000× | 1.000–1.000× | baseline |
| push_preallocated/4 | JackVec | 1.860 | 0.680× | 0.657–0.686× | win |
| push_preallocated/4 | ThinVec | 2.737 | 1.006× | 0.996–1.029× | equivalent |
| push_preallocated/4 | SmallVec4 | 9.981 | 3.610× | 3.434–3.722× | loss |
| push_preallocated/4 | SmallVec8 | 9.856 | 3.654× | 3.470–3.744× | loss |
| resize_reserved_1024 | Vec | 276.874 | 1.000× | 1.000–1.000× | baseline |
| resize_reserved_1024 | JackVec | 218.829 | 0.777× | 0.761–0.792× | win |
| resize_reserved_1024 | ThinVec | 688.958 | 2.488× | 2.314–2.658× | loss |
| resize_reserved_1024 | SmallVec4 | 285.867 | 0.996× | 0.958–1.070× | inconclusive |
| resize_reserved_1024 | SmallVec8 | 288.861 | 1.024× | 0.973–1.072× | inconclusive |
| retain_mixed/64_byte | Vec | 350.562 | 1.000× | 1.000–1.000× | baseline |
| retain_mixed/64_byte | JackVec | 372.417 | 1.070× | 1.042–1.102× | loss |
| retain_mixed/64_byte | ThinVec | 401.139 | 1.146× | 1.125–1.207× | loss |
| retain_mixed/64_byte | SmallVec4 | 498.510 | 1.419× | 1.387–1.501× | loss |
| retain_mixed/64_byte | SmallVec8 | 499.666 | 1.432× | 1.407–1.487× | loss |
| retain_mixed/u64 | Vec | 507.010 | 1.000× | 1.000–1.000× | baseline |
| retain_mixed/u64 | JackVec | 582.399 | 1.150× | 1.113–1.172× | loss |
| retain_mixed/u64 | ThinVec | 688.725 | 1.358× | 1.341–1.366× | loss |
| retain_mixed/u64 | SmallVec4 | 792.442 | 1.567× | 1.552–1.579× | loss |
| retain_mixed/u64 | SmallVec8 | 793.994 | 1.562× | 1.562–1.572× | loss |
| sequential_iteration/1024 | Vec | 59.407 | 1.000× | 1.000–1.000× | baseline |
| sequential_iteration/1024 | JackVec | 59.165 | 1.012× | 0.986–1.237× | inconclusive |
| sequential_iteration/1024 | ThinVec | 59.728 | 1.006× | 0.984–1.073× | inconclusive |
| sequential_iteration/1024 | SmallVec4 | 58.999 | 0.994× | 0.972–1.013× | equivalent |
| sequential_iteration/1024 | SmallVec8 | 59.428 | 1.000× | 0.984–1.013× | equivalent |
| sequential_iteration/8 | Vec | 1.594 | 1.000× | 1.000–1.000× | baseline |
| sequential_iteration/8 | JackVec | 1.594 | 1.000× | 0.999–1.011× | equivalent |
| sequential_iteration/8 | ThinVec | 1.594 | 1.000× | 1.000–1.020× | equivalent |
| sequential_iteration/8 | SmallVec4 | 1.594 | 0.999× | 0.995–1.000× | equivalent |
| sequential_iteration/8 | SmallVec8 | 1.594 | 1.000× | 1.000–1.020× | equivalent |

## Allocations

Owner bytes describe the collection values themselves. Requested and usable bytes describe allocator-visible storage; they must not be added together for nested workloads.

| Benchmark | Input | Element B | Implementation | Owner B | Live requested B | Live usable B | Allocs | Reallocs | Spilled |
|---|---:|---:|---|---:|---:|---:|---:|---:|---:|
| nested | empty | 8 | Vec | 240000 | 240000 | 262144 | 1 | 0 | na |
| nested | empty | 8 | JackVec | 80000 | 80000 | 98304 | 1 | 0 | na |
| nested | empty | 8 | ThinVec | 80000 | 80000 | 98304 | 1 | 0 | na |
| nested | empty | 8 | SmallVec4 | 400000 | 400000 | 425984 | 1 | 0 | 0 |
| nested | empty | 8 | SmallVec8 | 720000 | 720000 | 720896 | 1 | 0 | 0 |
| nested | sparse | 8 | Vec | 240000 | 304000 | 326144 | 2001 | 0 | na |
| nested | sparse | 8 | JackVec | 80000 | 160000 | 194304 | 2001 | 0 | na |
| nested | sparse | 8 | ThinVec | 80000 | 176000 | 194304 | 2001 | 0 | na |
| nested | sparse | 8 | SmallVec4 | 400000 | 400000 | 425984 | 1 | 0 | 0 |
| nested | sparse | 8 | SmallVec8 | 720000 | 720000 | 720896 | 1 | 0 | 0 |
| nested | small | 8 | Vec | 240000 | 560000 | 582144 | 10001 | 0 | na |
| nested | small | 8 | JackVec | 80000 | 480000 | 578304 | 10001 | 0 | na |
| nested | small | 8 | ThinVec | 80000 | 560000 | 578304 | 10001 | 0 | na |
| nested | small | 8 | SmallVec4 | 400000 | 400000 | 425984 | 1 | 0 | 0 |
| nested | small | 8 | SmallVec8 | 720000 | 720000 | 720896 | 1 | 0 | 0 |
| build_growing | 1 | 8 | Vec | 24 | 32 | 32 | 1 | 0 | na |
| build_growing | 1 | 8 | JackVec | 8 | 40 | 48 | 1 | 0 | na |
| build_growing | 1 | 8 | ThinVec | 8 | 48 | 48 | 1 | 0 | na |
| build_growing | 1 | 8 | SmallVec4 | 40 | 0 | 0 | 0 | 0 | 0 |
| build_growing | 1 | 8 | SmallVec8 | 72 | 0 | 0 | 0 | 0 | 0 |
| push_reserved | 1 | 8 | Vec | 24 | 8 | 16 | 1 | 0 | na |
| push_reserved | 1 | 8 | JackVec | 8 | 16 | 16 | 1 | 0 | na |
| push_reserved | 1 | 8 | ThinVec | 8 | 24 | 32 | 1 | 0 | na |
| push_reserved | 1 | 8 | SmallVec4 | 40 | 0 | 0 | 0 | 0 | 0 |
| push_reserved | 1 | 8 | SmallVec8 | 72 | 0 | 0 | 0 | 0 | 0 |
| push_reserved | 1 | 1 | Vec | 24 | 1 | 16 | 1 | 0 | na |
| push_reserved | 1 | 1 | JackVec | 8 | 9 | 16 | 1 | 0 | na |
| push_reserved | 1 | 1 | ThinVec | 8 | 17 | 32 | 1 | 0 | na |
| push_reserved | 1 | 1 | SmallVec4 | 24 | 0 | 0 | 0 | 0 | 0 |
| push_reserved | 1 | 1 | SmallVec8 | 24 | 0 | 0 | 0 | 0 | 0 |
| build_growing | 4 | 8 | Vec | 24 | 32 | 32 | 1 | 0 | na |
| build_growing | 4 | 8 | JackVec | 8 | 40 | 48 | 1 | 0 | na |
| build_growing | 4 | 8 | ThinVec | 8 | 48 | 48 | 1 | 0 | na |
| build_growing | 4 | 8 | SmallVec4 | 40 | 0 | 0 | 0 | 0 | 0 |
| build_growing | 4 | 8 | SmallVec8 | 72 | 0 | 0 | 0 | 0 | 0 |
| push_reserved | 4 | 8 | Vec | 24 | 32 | 32 | 1 | 0 | na |
| push_reserved | 4 | 8 | JackVec | 8 | 40 | 48 | 1 | 0 | na |
| push_reserved | 4 | 8 | ThinVec | 8 | 48 | 48 | 1 | 0 | na |
| push_reserved | 4 | 8 | SmallVec4 | 40 | 0 | 0 | 0 | 0 | 0 |
| push_reserved | 4 | 8 | SmallVec8 | 72 | 0 | 0 | 0 | 0 | 0 |
| push_reserved | 4 | 1 | Vec | 24 | 4 | 16 | 1 | 0 | na |
| push_reserved | 4 | 1 | JackVec | 8 | 12 | 16 | 1 | 0 | na |
| push_reserved | 4 | 1 | ThinVec | 8 | 20 | 32 | 1 | 0 | na |
| push_reserved | 4 | 1 | SmallVec4 | 24 | 0 | 0 | 0 | 0 | 0 |
| push_reserved | 4 | 1 | SmallVec8 | 24 | 0 | 0 | 0 | 0 | 0 |
| build_growing | 1024 | 8 | Vec | 24 | 8192 | 8192 | 1 | 8 | na |
| build_growing | 1024 | 8 | JackVec | 8 | 8200 | 8704 | 1 | 8 | na |
| build_growing | 1024 | 8 | ThinVec | 8 | 8208 | 8704 | 1 | 8 | na |
| build_growing | 1024 | 8 | SmallVec4 | 40 | 8192 | 8192 | 1 | 7 | 1 |
| build_growing | 1024 | 8 | SmallVec8 | 72 | 8192 | 8192 | 1 | 6 | 1 |
| push_reserved | 1024 | 8 | Vec | 24 | 8192 | 8192 | 1 | 0 | na |
| push_reserved | 1024 | 8 | JackVec | 8 | 8200 | 8704 | 1 | 0 | na |
| push_reserved | 1024 | 8 | ThinVec | 8 | 8208 | 8704 | 1 | 0 | na |
| push_reserved | 1024 | 8 | SmallVec4 | 40 | 8192 | 8192 | 1 | 0 | 1 |
| push_reserved | 1024 | 8 | SmallVec8 | 72 | 8192 | 8192 | 1 | 0 | 1 |
| push_reserved | 1024 | 1 | Vec | 24 | 1024 | 1024 | 1 | 0 | na |
| push_reserved | 1024 | 1 | JackVec | 8 | 1032 | 1536 | 1 | 0 | na |
| push_reserved | 1024 | 1 | ThinVec | 8 | 1040 | 1536 | 1 | 0 | na |
| push_reserved | 1024 | 1 | SmallVec4 | 24 | 1024 | 1024 | 1 | 0 | 1 |
| push_reserved | 1024 | 1 | SmallVec8 | 24 | 1024 | 1024 | 1 | 0 | 1 |
