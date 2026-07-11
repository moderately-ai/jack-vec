# JackVec comparison: linux-x86_64

- Commit: `2da08eea786c9755fa6ac883026ebfc8eeeb904e`
- Rust: `rustc 1.97.0 (2d8144b78 2026-07-07)`
- Platform: `Linux-5.15.0-177-generic-x86_64-with-glibc2.35`
- Allocator policy: `system`; effective override environment: `{'LD_PRELOAD': None, 'DYLD_INSERT_LIBRARIES': None}`
Rounds: 5; practical-equivalence band: 0.97–1.03× Vec.

A win or loss requires the complete paired bootstrap interval to clear the practical-equivalence band. Results that cross a boundary are reported as inconclusive.

## CPU

| Benchmark | Implementation | Median ns | Ratio | 95% interval | Result |
|---|---:|---:|---:|---:|---|
| append_preallocated/1024 | Vec | 160.173 | 1.000× | 1.000–1.000× | baseline |
| append_preallocated/1024 | JackVec | 163.412 | 1.018× | 1.017–1.033× | inconclusive |
| append_preallocated/1024 | ThinVec | 521.598 | 3.271× | 3.205–3.273× | loss |
| append_preallocated/1024 | SmallVec4 | 210.064 | 1.316× | 1.285–1.320× | loss |
| append_preallocated/1024 | SmallVec8 | 196.384 | 1.231× | 1.217–1.240× | loss |
| append_preallocated/4 | Vec | 2.794 | 1.000× | 1.000–1.000× | baseline |
| append_preallocated/4 | JackVec | 3.728 | 1.330× | 1.300–1.348× | loss |
| append_preallocated/4 | ThinVec | 4.318 | 1.551× | 1.538–1.557× | loss |
| append_preallocated/4 | SmallVec4 | 5.722 | 2.047× | 2.031–2.061× | loss |
| append_preallocated/4 | SmallVec8 | 5.704 | 2.049× | 2.019–2.148× | loss |
| build_growing_and_drop/1 | Vec | 11.491 | 1.000× | 1.000–1.000× | baseline |
| build_growing_and_drop/1 | JackVec | 10.905 | 0.947× | 0.917–0.959× | win |
| build_growing_and_drop/1 | ThinVec | 10.516 | 0.907× | 0.882–0.940× | win |
| build_growing_and_drop/1 | SmallVec4 | 2.054 | 0.179× | 0.177–0.180× | win |
| build_growing_and_drop/1 | SmallVec8 | 3.213 | 0.278× | 0.275–0.283× | win |
| build_growing_and_drop/1024 | Vec | 699.856 | 1.000× | 1.000–1.000× | baseline |
| build_growing_and_drop/1024 | JackVec | 611.927 | 0.874× | 0.785–1.069× | inconclusive |
| build_growing_and_drop/1024 | ThinVec | 557.191 | 0.804× | 0.763–0.968× | win |
| build_growing_and_drop/1024 | SmallVec4 | 933.890 | 1.476× | 1.325–1.619× | loss |
| build_growing_and_drop/1024 | SmallVec8 | 929.484 | 1.432× | 1.278–1.495× | loss |
| build_growing_and_drop/4 | Vec | 12.383 | 1.000× | 1.000–1.000× | baseline |
| build_growing_and_drop/4 | JackVec | 11.622 | 0.938× | 0.922–0.940× | win |
| build_growing_and_drop/4 | ThinVec | 11.368 | 0.910× | 0.909–0.954× | win |
| build_growing_and_drop/4 | SmallVec4 | 10.901 | 0.880× | 0.858–0.882× | win |
| build_growing_and_drop/4 | SmallVec8 | 9.186 | 0.742× | 0.693–0.794× | win |
| dedup_adjacent_pairs/64_byte | Vec | 972.739 | 1.000× | 1.000–1.000× | baseline |
| dedup_adjacent_pairs/64_byte | JackVec | 1070.404 | 1.100× | 1.098–1.100× | loss |
| dedup_adjacent_pairs/64_byte | ThinVec | 1097.902 | 1.130× | 1.081–1.134× | loss |
| dedup_adjacent_pairs/64_byte | SmallVec4 | 1099.313 | 1.131× | 1.127–1.139× | loss |
| dedup_adjacent_pairs/64_byte | SmallVec8 | 1098.761 | 1.131× | 1.128–1.135× | loss |
| dedup_adjacent_pairs/u64 | Vec | 307.613 | 1.000× | 1.000–1.000× | baseline |
| dedup_adjacent_pairs/u64 | JackVec | 328.872 | 1.069× | 1.042–1.096× | loss |
| dedup_adjacent_pairs/u64 | ThinVec | 524.800 | 1.693× | 1.387–1.717× | loss |
| dedup_adjacent_pairs/u64 | SmallVec4 | 396.146 | 1.281× | 1.250–1.323× | loss |
| dedup_adjacent_pairs/u64 | SmallVec8 | 397.971 | 1.290× | 1.263–1.332× | loss |
| extend_reserved_1024 | Vec | 154.130 | 1.000× | 1.000–1.000× | baseline |
| extend_reserved_1024 | JackVec | 159.464 | 1.035× | 1.012–1.053× | inconclusive |
| extend_reserved_1024 | ThinVec | 149.007 | 0.964× | 0.938–0.972× | inconclusive |
| extend_reserved_1024 | SmallVec4 | 184.075 | 1.194× | 1.160–1.200× | loss |
| extend_reserved_1024 | SmallVec8 | 182.017 | 1.183× | 1.146–1.189× | loss |
| nested_construct_and_drop/empty | Vec | 18959.341 | 1.000× | 1.000–1.000× | baseline |
| nested_construct_and_drop/empty | JackVec | 9642.449 | 0.511× | 0.500–0.568× | win |
| nested_construct_and_drop/empty | ThinVec | 10747.226 | 0.570× | 0.565–0.576× | win |
| nested_construct_and_drop/empty | SmallVec4 | 22251.789 | 1.176× | 1.170–1.204× | loss |
| nested_construct_and_drop/empty | SmallVec8 | 27187.117 | 1.447× | 1.426–1.458× | loss |
| nested_construct_and_drop/small | Vec | 226517.114 | 1.000× | 1.000–1.000× | baseline |
| nested_construct_and_drop/small | JackVec | 213288.409 | 0.944× | 0.897–0.946× | win |
| nested_construct_and_drop/small | ThinVec | 214035.345 | 0.946× | 0.894–0.948× | win |
| nested_construct_and_drop/small | SmallVec4 | 128721.890 | 0.568× | 0.557–0.571× | win |
| nested_construct_and_drop/small | SmallVec8 | 131863.728 | 0.583× | 0.568–0.586× | win |
| nested_construct_and_drop/sparse | Vec | 63121.480 | 1.000× | 1.000–1.000× | baseline |
| nested_construct_and_drop/sparse | JackVec | 56704.910 | 0.892× | 0.887–0.914× | win |
| nested_construct_and_drop/sparse | ThinVec | 54584.925 | 0.862× | 0.848–0.878× | win |
| nested_construct_and_drop/sparse | SmallVec4 | 40560.407 | 0.642× | 0.629–0.654× | win |
| nested_construct_and_drop/sparse | SmallVec8 | 45610.577 | 0.717× | 0.710–0.726× | win |
| nested_metadata_scan_sparse | Vec | 3973.003 | 1.000× | 1.000–1.000× | baseline |
| nested_metadata_scan_sparse | JackVec | 4003.828 | 1.010× | 1.007–1.015× | equivalent |
| nested_metadata_scan_sparse | ThinVec | 4003.677 | 1.007× | 1.006–1.013× | equivalent |
| nested_metadata_scan_sparse | SmallVec4 | 4156.714 | 1.051× | 1.016–1.058× | inconclusive |
| nested_metadata_scan_sparse | SmallVec8 | 5112.391 | 1.292× | 1.250–1.340× | loss |
| nested_traverse/empty | Vec | 2191.751 | 1.000× | 1.000–1.000× | baseline |
| nested_traverse/empty | JackVec | 2153.251 | 0.982× | 0.982–0.985× | equivalent |
| nested_traverse/empty | ThinVec | 2155.136 | 0.983× | 0.982–0.985× | equivalent |
| nested_traverse/empty | SmallVec4 | 5875.006 | 2.681× | 2.679–2.692× | loss |
| nested_traverse/empty | SmallVec8 | 6052.416 | 2.761× | 2.730–2.767× | loss |
| nested_traverse/small | Vec | 21414.993 | 1.000× | 1.000–1.000× | baseline |
| nested_traverse/small | JackVec | 21420.348 | 1.000× | 1.000–1.009× | equivalent |
| nested_traverse/small | ThinVec | 21424.411 | 1.000× | 0.996–1.004× | equivalent |
| nested_traverse/small | SmallVec4 | 21418.573 | 1.000× | 1.000–1.000× | equivalent |
| nested_traverse/small | SmallVec8 | 21419.927 | 1.000× | 0.999–1.001× | equivalent |
| nested_traverse/sparse | Vec | 3087.824 | 1.000× | 1.000–1.000× | baseline |
| nested_traverse/sparse | JackVec | 3082.562 | 0.999× | 0.983–1.012× | equivalent |
| nested_traverse/sparse | ThinVec | 3092.109 | 0.999× | 0.993–1.026× | equivalent |
| nested_traverse/sparse | SmallVec4 | 6880.874 | 2.228× | 2.213–2.233× | loss |
| nested_traverse/sparse | SmallVec8 | 5362.402 | 1.732× | 1.714–1.805× | loss |
| push_preallocated/1024 | Vec | 448.354 | 1.000× | 1.000–1.000× | baseline |
| push_preallocated/1024 | JackVec | 427.537 | 0.952× | 0.950–0.954× | win |
| push_preallocated/1024 | ThinVec | 421.012 | 0.938× | 0.925–0.944× | win |
| push_preallocated/1024 | SmallVec4 | 718.665 | 1.603× | 1.593–1.609× | loss |
| push_preallocated/1024 | SmallVec8 | 724.131 | 1.620× | 1.612–1.638× | loss |
| push_preallocated/4 | Vec | 2.442 | 1.000× | 1.000–1.000× | baseline |
| push_preallocated/4 | JackVec | 2.151 | 0.882× | 0.859–0.897× | win |
| push_preallocated/4 | ThinVec | 2.254 | 0.930× | 0.905–0.936× | win |
| push_preallocated/4 | SmallVec4 | 14.155 | 5.812× | 5.657–5.940× | loss |
| push_preallocated/4 | SmallVec8 | 14.187 | 5.820× | 5.700–5.974× | loss |
| resize_reserved_1024 | Vec | 144.512 | 1.000× | 1.000–1.000× | baseline |
| resize_reserved_1024 | JackVec | 154.230 | 1.067× | 1.055–1.077× | loss |
| resize_reserved_1024 | ThinVec | 429.145 | 2.970× | 2.937–3.006× | loss |
| resize_reserved_1024 | SmallVec4 | 144.214 | 0.996× | 0.987–1.017× | equivalent |
| resize_reserved_1024 | SmallVec8 | 144.066 | 1.000× | 0.948–1.009× | inconclusive |
| retain_mixed/64_byte | Vec | 287.639 | 1.000× | 1.000–1.000× | baseline |
| retain_mixed/64_byte | JackVec | 316.699 | 1.105× | 1.100–1.120× | loss |
| retain_mixed/64_byte | ThinVec | 444.747 | 1.546× | 1.529–1.611× | loss |
| retain_mixed/64_byte | SmallVec4 | 553.008 | 1.926× | 1.889–1.967× | loss |
| retain_mixed/64_byte | SmallVec8 | 547.569 | 1.910× | 1.880–1.920× | loss |
| retain_mixed/u64 | Vec | 559.680 | 1.000× | 1.000–1.000× | baseline |
| retain_mixed/u64 | JackVec | 696.937 | 1.246× | 1.232–1.250× | loss |
| retain_mixed/u64 | ThinVec | 716.897 | 1.283× | 1.274–1.285× | loss |
| retain_mixed/u64 | SmallVec4 | 924.432 | 1.653× | 1.646–1.663× | loss |
| retain_mixed/u64 | SmallVec8 | 926.110 | 1.654× | 1.641–1.663× | loss |
| sequential_iteration/1024 | Vec | 102.544 | 1.000× | 1.000–1.000× | baseline |
| sequential_iteration/1024 | JackVec | 102.520 | 1.000× | 0.998–1.005× | equivalent |
| sequential_iteration/1024 | ThinVec | 102.541 | 1.000× | 1.000–1.002× | equivalent |
| sequential_iteration/1024 | SmallVec4 | 102.519 | 1.000× | 0.998–1.000× | equivalent |
| sequential_iteration/1024 | SmallVec8 | 102.518 | 1.000× | 0.998–1.000× | equivalent |
| sequential_iteration/8 | Vec | 1.813 | 1.000× | 1.000–1.000× | baseline |
| sequential_iteration/8 | JackVec | 1.657 | 0.914× | 0.909–0.915× | win |
| sequential_iteration/8 | ThinVec | 1.817 | 1.003× | 1.002–1.016× | equivalent |
| sequential_iteration/8 | SmallVec4 | 1.654 | 0.912× | 0.908–0.914× | win |
| sequential_iteration/8 | SmallVec8 | 1.658 | 0.915× | 0.907–0.917× | win |

## Allocations

Owner bytes describe the collection values themselves. Requested and usable bytes describe allocator-visible storage; they must not be added together for nested workloads.

| Benchmark | Input | Element B | Implementation | Owner B | Live requested B | Live usable B | Allocs | Reallocs | Spilled |
|---|---:|---:|---|---:|---:|---:|---:|---:|---:|
| nested | empty | 8 | Vec | 240000 | 240000 | 241648 | 1 | 0 | na |
| nested | empty | 8 | JackVec | 80000 | 80000 | 80008 | 1 | 0 | na |
| nested | empty | 8 | ThinVec | 80000 | 80000 | 80008 | 1 | 0 | na |
| nested | empty | 8 | SmallVec4 | 400000 | 400000 | 401392 | 1 | 0 | 0 |
| nested | empty | 8 | SmallVec8 | 720000 | 720000 | 720880 | 1 | 0 | 0 |
| nested | sparse | 8 | Vec | 240000 | 304000 | 320008 | 2001 | 0 | na |
| nested | sparse | 8 | JackVec | 80000 | 160000 | 160008 | 2001 | 0 | na |
| nested | sparse | 8 | ThinVec | 80000 | 176000 | 192008 | 2001 | 0 | na |
| nested | sparse | 8 | SmallVec4 | 400000 | 400000 | 400008 | 1 | 0 | 0 |
| nested | sparse | 8 | SmallVec8 | 720000 | 720000 | 720008 | 1 | 0 | 0 |
| nested | small | 8 | Vec | 240000 | 560000 | 640024 | 10001 | 0 | na |
| nested | small | 8 | JackVec | 80000 | 480000 | 480024 | 10001 | 0 | na |
| nested | small | 8 | ThinVec | 80000 | 560000 | 640024 | 10001 | 0 | na |
| nested | small | 8 | SmallVec4 | 400000 | 400000 | 400008 | 1 | 0 | 0 |
| nested | small | 8 | SmallVec8 | 720000 | 720000 | 720008 | 1 | 0 | 0 |
| build_growing | 1 | 8 | Vec | 24 | 32 | 40 | 1 | 0 | na |
| build_growing | 1 | 8 | JackVec | 8 | 40 | 40 | 1 | 0 | na |
| build_growing | 1 | 8 | ThinVec | 8 | 48 | 56 | 1 | 0 | na |
| build_growing | 1 | 8 | SmallVec4 | 40 | 0 | 0 | 0 | 0 | 0 |
| build_growing | 1 | 8 | SmallVec8 | 72 | 0 | 0 | 0 | 0 | 0 |
| push_reserved | 1 | 8 | Vec | 24 | 8 | 24 | 1 | 0 | na |
| push_reserved | 1 | 8 | JackVec | 8 | 16 | 24 | 1 | 0 | na |
| push_reserved | 1 | 8 | ThinVec | 8 | 24 | 24 | 1 | 0 | na |
| push_reserved | 1 | 8 | SmallVec4 | 40 | 0 | 0 | 0 | 0 | 0 |
| push_reserved | 1 | 8 | SmallVec8 | 72 | 0 | 0 | 0 | 0 | 0 |
| push_reserved | 1 | 1 | Vec | 24 | 1 | 24 | 1 | 0 | na |
| push_reserved | 1 | 1 | JackVec | 8 | 9 | 24 | 1 | 0 | na |
| push_reserved | 1 | 1 | ThinVec | 8 | 17 | 24 | 1 | 0 | na |
| push_reserved | 1 | 1 | SmallVec4 | 24 | 0 | 0 | 0 | 0 | 0 |
| push_reserved | 1 | 1 | SmallVec8 | 24 | 0 | 0 | 0 | 0 | 0 |
| build_growing | 4 | 8 | Vec | 24 | 32 | 40 | 1 | 0 | na |
| build_growing | 4 | 8 | JackVec | 8 | 40 | 40 | 1 | 0 | na |
| build_growing | 4 | 8 | ThinVec | 8 | 48 | 56 | 1 | 0 | na |
| build_growing | 4 | 8 | SmallVec4 | 40 | 0 | 0 | 0 | 0 | 0 |
| build_growing | 4 | 8 | SmallVec8 | 72 | 0 | 0 | 0 | 0 | 0 |
| push_reserved | 4 | 8 | Vec | 24 | 32 | 40 | 1 | 0 | na |
| push_reserved | 4 | 8 | JackVec | 8 | 40 | 40 | 1 | 0 | na |
| push_reserved | 4 | 8 | ThinVec | 8 | 48 | 56 | 1 | 0 | na |
| push_reserved | 4 | 8 | SmallVec4 | 40 | 0 | 0 | 0 | 0 | 0 |
| push_reserved | 4 | 8 | SmallVec8 | 72 | 0 | 0 | 0 | 0 | 0 |
| push_reserved | 4 | 1 | Vec | 24 | 4 | 24 | 1 | 0 | na |
| push_reserved | 4 | 1 | JackVec | 8 | 12 | 24 | 1 | 0 | na |
| push_reserved | 4 | 1 | ThinVec | 8 | 20 | 24 | 1 | 0 | na |
| push_reserved | 4 | 1 | SmallVec4 | 24 | 0 | 0 | 0 | 0 | 0 |
| push_reserved | 4 | 1 | SmallVec8 | 24 | 0 | 0 | 0 | 0 | 0 |
| build_growing | 1024 | 8 | Vec | 24 | 8192 | 8200 | 1 | 8 | na |
| build_growing | 1024 | 8 | JackVec | 8 | 8200 | 8200 | 1 | 8 | na |
| build_growing | 1024 | 8 | ThinVec | 8 | 8208 | 8216 | 1 | 8 | na |
| build_growing | 1024 | 8 | SmallVec4 | 40 | 8192 | 8200 | 1 | 7 | 1 |
| build_growing | 1024 | 8 | SmallVec8 | 72 | 8192 | 8200 | 1 | 6 | 1 |
| push_reserved | 1024 | 8 | Vec | 24 | 8192 | 8200 | 1 | 0 | na |
| push_reserved | 1024 | 8 | JackVec | 8 | 8200 | 8200 | 1 | 0 | na |
| push_reserved | 1024 | 8 | ThinVec | 8 | 8208 | 8216 | 1 | 0 | na |
| push_reserved | 1024 | 8 | SmallVec4 | 40 | 8192 | 8200 | 1 | 0 | 1 |
| push_reserved | 1024 | 8 | SmallVec8 | 72 | 8192 | 8200 | 1 | 0 | 1 |
| push_reserved | 1024 | 1 | Vec | 24 | 1024 | 1032 | 1 | 0 | na |
| push_reserved | 1024 | 1 | JackVec | 8 | 1032 | 1032 | 1 | 0 | na |
| push_reserved | 1024 | 1 | ThinVec | 8 | 1040 | 1048 | 1 | 0 | na |
| push_reserved | 1024 | 1 | SmallVec4 | 24 | 1024 | 1032 | 1 | 0 | 1 |
| push_reserved | 1024 | 1 | SmallVec8 | 24 | 1024 | 1032 | 1 | 0 | 1 |
