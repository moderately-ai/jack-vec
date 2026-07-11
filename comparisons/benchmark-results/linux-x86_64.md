# JackVec comparison: linux-x86_64

Commit: `1f6e5f8abac7152d6ef3523baaed63096ac88b0a`  
Rust: `rustc 1.97.0 (2d8144b78 2026-07-07)`  
Platform: `Linux-5.15.0-177-generic-x86_64-with-glibc2.35`  
Rounds: 5; practical-equivalence band: 0.97–1.03× Vec.

A win or loss requires the complete paired bootstrap interval to clear the practical-equivalence band. Results that cross a boundary are reported as inconclusive.

## CPU

| Benchmark | Implementation | Median ns | Ratio | 95% interval | Result |
|---|---:|---:|---:|---:|---|
| append_preallocated/1024 | Vec | 161.775 | 1.000× | 1.000–1.000× | baseline |
| append_preallocated/1024 | JackVec | 163.955 | 1.015× | 0.997–1.022× | equivalent |
| append_preallocated/1024 | ThinVec | 508.776 | 3.145× | 3.126–3.193× | loss |
| append_preallocated/1024 | SmallVec4 | 192.696 | 1.194× | 1.177–1.228× | loss |
| append_preallocated/1024 | SmallVec8 | 208.087 | 1.287× | 1.278–1.299× | loss |
| append_preallocated/4 | Vec | 2.589 | 1.000× | 1.000–1.000× | baseline |
| append_preallocated/4 | JackVec | 3.719 | 1.436× | 1.433–1.482× | loss |
| append_preallocated/4 | ThinVec | 4.116 | 1.586× | 1.567–1.618× | loss |
| append_preallocated/4 | SmallVec4 | 5.723 | 2.211× | 2.205–2.214× | loss |
| append_preallocated/4 | SmallVec8 | 5.710 | 2.200× | 2.193–2.209× | loss |
| build_growing_and_drop/1 | Vec | 8.312 | 1.000× | 1.000–1.000× | baseline |
| build_growing_and_drop/1 | JackVec | 7.683 | 0.927× | 0.922–0.938× | win |
| build_growing_and_drop/1 | ThinVec | 7.659 | 0.922× | 0.921–0.927× | win |
| build_growing_and_drop/1 | SmallVec4 | 2.054 | 0.247× | 0.241–0.249× | win |
| build_growing_and_drop/1 | SmallVec8 | 3.205 | 0.379× | 0.376–0.388× | win |
| build_growing_and_drop/1024 | Vec | 678.735 | 1.000× | 1.000–1.000× | baseline |
| build_growing_and_drop/1024 | JackVec | 562.051 | 0.827× | 0.817–0.845× | win |
| build_growing_and_drop/1024 | ThinVec | 550.895 | 0.811× | 0.802–0.830× | win |
| build_growing_and_drop/1024 | SmallVec4 | 1002.856 | 1.478× | 1.466–1.510× | loss |
| build_growing_and_drop/1024 | SmallVec8 | 1006.023 | 1.475× | 1.427–1.507× | loss |
| build_growing_and_drop/4 | Vec | 9.266 | 1.000× | 1.000–1.000× | baseline |
| build_growing_and_drop/4 | JackVec | 8.947 | 0.969× | 0.957–0.980× | inconclusive |
| build_growing_and_drop/4 | ThinVec | 8.468 | 0.923× | 0.892–0.935× | win |
| build_growing_and_drop/4 | SmallVec4 | 10.868 | 1.173× | 1.169–1.189× | loss |
| build_growing_and_drop/4 | SmallVec8 | 8.724 | 0.941× | 0.928–1.009× | inconclusive |
| dedup_adjacent_pairs/64_byte | Vec | 967.355 | 1.000× | 1.000–1.000× | baseline |
| dedup_adjacent_pairs/64_byte | JackVec | 1039.235 | 1.073× | 1.071–1.076× | loss |
| dedup_adjacent_pairs/64_byte | ThinVec | 1006.277 | 1.040× | 1.039–1.042× | loss |
| dedup_adjacent_pairs/64_byte | SmallVec4 | 1144.656 | 1.183× | 1.179–1.198× | loss |
| dedup_adjacent_pairs/64_byte | SmallVec8 | 1147.124 | 1.186× | 1.182–1.197× | loss |
| dedup_adjacent_pairs/u64 | Vec | 267.912 | 1.000× | 1.000–1.000× | baseline |
| dedup_adjacent_pairs/u64 | JackVec | 282.549 | 1.059× | 1.002–1.067× | inconclusive |
| dedup_adjacent_pairs/u64 | ThinVec | 369.631 | 1.384× | 1.376–1.548× | loss |
| dedup_adjacent_pairs/u64 | SmallVec4 | 447.886 | 1.659× | 1.589–2.044× | loss |
| dedup_adjacent_pairs/u64 | SmallVec8 | 367.336 | 1.374× | 1.307–1.384× | loss |
| extend_reserved_1024 | Vec | 152.208 | 1.000× | 1.000–1.000× | baseline |
| extend_reserved_1024 | JackVec | 164.931 | 1.072× | 1.038–1.113× | loss |
| extend_reserved_1024 | ThinVec | 155.382 | 1.021× | 0.977–1.059× | inconclusive |
| extend_reserved_1024 | SmallVec4 | 148.660 | 1.001× | 0.929–1.021× | inconclusive |
| extend_reserved_1024 | SmallVec8 | 160.759 | 1.044× | 0.988–1.078× | inconclusive |
| nested_construct_and_drop/empty | Vec | 18932.942 | 1.000× | 1.000–1.000× | baseline |
| nested_construct_and_drop/empty | JackVec | 12071.659 | 0.635× | 0.575–0.639× | win |
| nested_construct_and_drop/empty | ThinVec | 10761.009 | 0.565× | 0.503–0.570× | win |
| nested_construct_and_drop/empty | SmallVec4 | 21602.297 | 1.143× | 1.127–1.154× | loss |
| nested_construct_and_drop/empty | SmallVec8 | 27141.307 | 1.437× | 1.432–1.443× | loss |
| nested_construct_and_drop/small | Vec | 113911.536 | 1.000× | 1.000–1.000× | baseline |
| nested_construct_and_drop/small | JackVec | 108169.508 | 0.948× | 0.935–0.955× | win |
| nested_construct_and_drop/small | ThinVec | 103041.828 | 0.905× | 0.891–0.917× | win |
| nested_construct_and_drop/small | SmallVec4 | 126057.111 | 1.107× | 1.101–1.112× | loss |
| nested_construct_and_drop/small | SmallVec8 | 133435.798 | 1.170× | 1.160–1.180× | loss |
| nested_construct_and_drop/sparse | Vec | 37194.696 | 1.000× | 1.000–1.000× | baseline |
| nested_construct_and_drop/sparse | JackVec | 32354.043 | 0.874× | 0.861–0.877× | win |
| nested_construct_and_drop/sparse | ThinVec | 30068.761 | 0.811× | 0.782–0.813× | win |
| nested_construct_and_drop/sparse | SmallVec4 | 40198.086 | 1.080× | 1.078–1.084× | loss |
| nested_construct_and_drop/sparse | SmallVec8 | 45716.869 | 1.227× | 1.202–1.241× | loss |
| nested_metadata_scan_sparse | Vec | 3971.005 | 1.000× | 1.000–1.000× | baseline |
| nested_metadata_scan_sparse | JackVec | 4025.529 | 1.015× | 1.013–1.057× | inconclusive |
| nested_metadata_scan_sparse | ThinVec | 4018.097 | 1.013× | 1.008–1.040× | inconclusive |
| nested_metadata_scan_sparse | SmallVec4 | 4085.277 | 1.028× | 1.027–1.063× | inconclusive |
| nested_metadata_scan_sparse | SmallVec8 | 5346.578 | 1.346× | 1.260–1.392× | loss |
| nested_traverse/empty | Vec | 2197.898 | 1.000× | 1.000–1.000× | baseline |
| nested_traverse/empty | JackVec | 2152.744 | 0.980× | 0.979–0.984× | equivalent |
| nested_traverse/empty | ThinVec | 3955.254 | 1.804× | 1.799–1.811× | loss |
| nested_traverse/empty | SmallVec4 | 5873.415 | 2.678× | 1.840–2.684× | loss |
| nested_traverse/empty | SmallVec8 | 6001.315 | 2.737× | 2.187–2.759× | loss |
| nested_traverse/small | Vec | 21533.945 | 1.000× | 1.000–1.000× | baseline |
| nested_traverse/small | JackVec | 21523.874 | 0.999× | 0.985–1.003× | equivalent |
| nested_traverse/small | ThinVec | 21521.787 | 0.999× | 0.985–1.000× | equivalent |
| nested_traverse/small | SmallVec4 | 21421.319 | 0.994× | 0.985–0.997× | equivalent |
| nested_traverse/small | SmallVec8 | 21429.866 | 0.995× | 0.983–0.998× | equivalent |
| nested_traverse/sparse | Vec | 3242.745 | 1.000× | 1.000–1.000× | baseline |
| nested_traverse/sparse | JackVec | 3055.728 | 0.945× | 0.827–0.994× | inconclusive |
| nested_traverse/sparse | ThinVec | 5241.989 | 1.623× | 1.380–1.706× | loss |
| nested_traverse/sparse | SmallVec4 | 5045.058 | 1.555× | 1.327–1.643× | loss |
| nested_traverse/sparse | SmallVec8 | 5493.643 | 1.667× | 1.476–1.754× | loss |
| push_preallocated/1024 | Vec | 439.968 | 1.000× | 1.000–1.000× | baseline |
| push_preallocated/1024 | JackVec | 432.513 | 0.986× | 0.977–0.998× | equivalent |
| push_preallocated/1024 | ThinVec | 428.796 | 0.974× | 0.964–0.983× | inconclusive |
| push_preallocated/1024 | SmallVec4 | 714.322 | 1.623× | 1.617–1.643× | loss |
| push_preallocated/1024 | SmallVec8 | 717.536 | 1.630× | 1.626–1.640× | loss |
| push_preallocated/4 | Vec | 2.274 | 1.000× | 1.000–1.000× | baseline |
| push_preallocated/4 | JackVec | 2.103 | 0.920× | 0.911–0.931× | win |
| push_preallocated/4 | ThinVec | 2.117 | 0.932× | 0.917–0.945× | win |
| push_preallocated/4 | SmallVec4 | 14.058 | 6.135× | 6.097–6.221× | loss |
| push_preallocated/4 | SmallVec8 | 14.102 | 6.207× | 6.105–6.253× | loss |
| resize_reserved_1024 | Vec | 154.655 | 1.000× | 1.000–1.000× | baseline |
| resize_reserved_1024 | JackVec | 165.592 | 1.071× | 1.014–1.107× | inconclusive |
| resize_reserved_1024 | ThinVec | 437.112 | 2.830× | 2.793–2.917× | loss |
| resize_reserved_1024 | SmallVec4 | 153.335 | 0.982× | 0.939–1.053× | inconclusive |
| resize_reserved_1024 | SmallVec8 | 154.313 | 1.004× | 0.941–1.039× | inconclusive |
| retain_mixed/64_byte | Vec | 252.494 | 1.000× | 1.000–1.000× | baseline |
| retain_mixed/64_byte | JackVec | 294.815 | 1.163× | 1.158–1.194× | loss |
| retain_mixed/64_byte | ThinVec | 445.460 | 1.768× | 1.760–1.785× | loss |
| retain_mixed/64_byte | SmallVec4 | 526.996 | 2.089× | 2.076–2.127× | loss |
| retain_mixed/64_byte | SmallVec8 | 531.974 | 2.110× | 2.098–2.120× | loss |
| retain_mixed/u64 | Vec | 558.547 | 1.000× | 1.000–1.000× | baseline |
| retain_mixed/u64 | JackVec | 696.019 | 1.247× | 1.244–1.250× | loss |
| retain_mixed/u64 | ThinVec | 710.670 | 1.271× | 1.266–1.275× | loss |
| retain_mixed/u64 | SmallVec4 | 919.615 | 1.645× | 1.643–1.651× | loss |
| retain_mixed/u64 | SmallVec8 | 926.020 | 1.656× | 1.654–1.662× | loss |
| sequential_iteration/1024 | Vec | 58.679 | 1.000× | 1.000–1.000× | baseline |
| sequential_iteration/1024 | JackVec | 74.200 | 1.265× | 1.263–1.268× | loss |
| sequential_iteration/1024 | ThinVec | 58.847 | 1.003× | 1.002–1.004× | equivalent |
| sequential_iteration/1024 | SmallVec4 | 101.139 | 1.724× | 1.723–1.732× | loss |
| sequential_iteration/1024 | SmallVec8 | 58.641 | 1.000× | 0.999–1.000× | equivalent |
| sequential_iteration/8 | Vec | 1.075 | 1.000× | 1.000–1.000× | baseline |
| sequential_iteration/8 | JackVec | 1.271 | 1.182× | 1.179–1.185× | loss |
| sequential_iteration/8 | ThinVec | 1.398 | 1.300× | 1.297–1.304× | loss |
| sequential_iteration/8 | SmallVec4 | 1.223 | 1.138× | 1.135–1.140× | loss |
| sequential_iteration/8 | SmallVec8 | 1.088 | 1.012× | 1.009–1.017× | equivalent |

## Allocations

Owner bytes describe the collection values themselves. Requested and usable bytes describe allocator-visible storage; they must not be added together for nested workloads.

| Benchmark | Input | Element B | Implementation | Owner B | Live requested B | Live usable B | Allocs | Reallocs | Spilled |
|---|---:|---:|---|---:|---:|---:|---:|---:|---:|
| nested | empty | 8 | Vec | 240000 | 240000 | 245760 | 1 | 0 | na |
| nested | empty | 8 | JackVec | 80000 | 80000 | 81920 | 1 | 0 | na |
| nested | empty | 8 | ThinVec | 80000 | 80000 | 81920 | 1 | 0 | na |
| nested | empty | 8 | SmallVec4 | 400000 | 400000 | 401408 | 1 | 0 | 0 |
| nested | empty | 8 | SmallVec8 | 720000 | 720000 | 720896 | 1 | 0 | 0 |
| nested | sparse | 8 | Vec | 240000 | 304000 | 309760 | 2001 | 0 | na |
| nested | sparse | 8 | JackVec | 80000 | 160000 | 177920 | 2001 | 0 | na |
| nested | sparse | 8 | ThinVec | 80000 | 176000 | 177920 | 2001 | 0 | na |
| nested | sparse | 8 | SmallVec4 | 400000 | 400000 | 401408 | 1 | 0 | 0 |
| nested | sparse | 8 | SmallVec8 | 720000 | 720000 | 720896 | 1 | 0 | 0 |
| nested | small | 8 | Vec | 240000 | 560000 | 565760 | 10001 | 0 | na |
| nested | small | 8 | JackVec | 80000 | 480000 | 561920 | 10001 | 0 | na |
| nested | small | 8 | ThinVec | 80000 | 560000 | 561920 | 10001 | 0 | na |
| nested | small | 8 | SmallVec4 | 400000 | 400000 | 401408 | 1 | 0 | 0 |
| nested | small | 8 | SmallVec8 | 720000 | 720000 | 720896 | 1 | 0 | 0 |
| build_growing | 1 | 8 | Vec | 24 | 32 | 32 | 1 | 0 | na |
| build_growing | 1 | 8 | JackVec | 8 | 40 | 48 | 1 | 0 | na |
| build_growing | 1 | 8 | ThinVec | 8 | 48 | 48 | 1 | 0 | na |
| build_growing | 1 | 8 | SmallVec4 | 40 | 0 | 0 | 0 | 0 | 0 |
| build_growing | 1 | 8 | SmallVec8 | 72 | 0 | 0 | 0 | 0 | 0 |
| push_reserved | 1 | 8 | Vec | 24 | 8 | 8 | 1 | 0 | na |
| push_reserved | 1 | 8 | JackVec | 8 | 16 | 16 | 1 | 0 | na |
| push_reserved | 1 | 8 | ThinVec | 8 | 24 | 32 | 1 | 0 | na |
| push_reserved | 1 | 8 | SmallVec4 | 40 | 0 | 0 | 0 | 0 | 0 |
| push_reserved | 1 | 8 | SmallVec8 | 72 | 0 | 0 | 0 | 0 | 0 |
| push_reserved | 1 | 1 | Vec | 24 | 1 | 8 | 1 | 0 | na |
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
| push_reserved | 4 | 1 | Vec | 24 | 4 | 8 | 1 | 0 | na |
| push_reserved | 4 | 1 | JackVec | 8 | 12 | 16 | 1 | 0 | na |
| push_reserved | 4 | 1 | ThinVec | 8 | 20 | 32 | 1 | 0 | na |
| push_reserved | 4 | 1 | SmallVec4 | 24 | 0 | 0 | 0 | 0 | 0 |
| push_reserved | 4 | 1 | SmallVec8 | 24 | 0 | 0 | 0 | 0 | 0 |
| build_growing | 1024 | 8 | Vec | 24 | 8192 | 8192 | 1 | 8 | na |
| build_growing | 1024 | 8 | JackVec | 8 | 8200 | 9216 | 1 | 8 | na |
| build_growing | 1024 | 8 | ThinVec | 8 | 8208 | 9216 | 1 | 8 | na |
| build_growing | 1024 | 8 | SmallVec4 | 40 | 8192 | 8192 | 1 | 7 | 1 |
| build_growing | 1024 | 8 | SmallVec8 | 72 | 8192 | 8192 | 1 | 6 | 1 |
| push_reserved | 1024 | 8 | Vec | 24 | 8192 | 8192 | 1 | 0 | na |
| push_reserved | 1024 | 8 | JackVec | 8 | 8200 | 9216 | 1 | 0 | na |
| push_reserved | 1024 | 8 | ThinVec | 8 | 8208 | 9216 | 1 | 0 | na |
| push_reserved | 1024 | 8 | SmallVec4 | 40 | 8192 | 8192 | 1 | 0 | 1 |
| push_reserved | 1024 | 8 | SmallVec8 | 72 | 8192 | 8192 | 1 | 0 | 1 |
| push_reserved | 1024 | 1 | Vec | 24 | 1024 | 1024 | 1 | 0 | na |
| push_reserved | 1024 | 1 | JackVec | 8 | 1032 | 1152 | 1 | 0 | na |
| push_reserved | 1024 | 1 | ThinVec | 8 | 1040 | 1152 | 1 | 0 | na |
| push_reserved | 1024 | 1 | SmallVec4 | 24 | 1024 | 1024 | 1 | 0 | 1 |
| push_reserved | 1024 | 1 | SmallVec8 | 24 | 1024 | 1024 | 1 | 0 | 1 |
