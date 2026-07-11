# JackVec comparison: linux-x86_64

- Commit: `2dc82e0656ba2c3997c4dc2dd26b905ce621bb2f`
- Rust: `rustc 1.97.0 (2d8144b78 2026-07-07)`
- Platform: `Linux-5.15.0-177-generic-x86_64-with-glibc2.35`
- Allocator policy: `system`; effective override environment: `{'LD_PRELOAD': None, 'DYLD_INSERT_LIBRARIES': None}`
Rounds: 5; practical-equivalence band: 0.97–1.03× Vec.

A win or loss requires the complete paired bootstrap interval to clear the practical-equivalence band. Results that cross a boundary are reported as inconclusive.

## CPU

| Benchmark | Implementation | Median ns | Ratio | 95% interval | Result |
|---|---:|---:|---:|---:|---|
| append_preallocated/1024 | Vec | 159.122 | 1.000× | 1.000–1.000× | baseline |
| append_preallocated/1024 | JackVec | 163.075 | 1.025× | 1.023–1.029× | equivalent |
| append_preallocated/1024 | ThinVec | 524.070 | 3.279× | 3.237–3.307× | loss |
| append_preallocated/1024 | SmallVec4 | 207.899 | 1.308× | 1.302–1.319× | loss |
| append_preallocated/1024 | SmallVec8 | 197.359 | 1.238× | 1.229–1.246× | loss |
| append_preallocated/4 | Vec | 2.773 | 1.000× | 1.000–1.000× | baseline |
| append_preallocated/4 | JackVec | 3.668 | 1.313× | 1.311–1.365× | loss |
| append_preallocated/4 | ThinVec | 4.331 | 1.560× | 1.549–1.597× | loss |
| append_preallocated/4 | SmallVec4 | 5.729 | 2.067× | 2.044–2.109× | loss |
| append_preallocated/4 | SmallVec8 | 5.739 | 2.077× | 2.047–2.167× | loss |
| build_growing_and_drop/1 | Vec | 11.529 | 1.000× | 1.000–1.000× | baseline |
| build_growing_and_drop/1 | JackVec | 10.967 | 0.948× | 0.912–0.974× | inconclusive |
| build_growing_and_drop/1 | ThinVec | 10.679 | 0.928× | 0.893–0.938× | win |
| build_growing_and_drop/1 | SmallVec4 | 2.055 | 0.178× | 0.172–0.179× | win |
| build_growing_and_drop/1 | SmallVec8 | 3.207 | 0.273× | 0.263–0.280× | win |
| build_growing_and_drop/1024 | Vec | 718.999 | 1.000× | 1.000–1.000× | baseline |
| build_growing_and_drop/1024 | JackVec | 600.466 | 0.904× | 0.803–0.910× | win |
| build_growing_and_drop/1024 | ThinVec | 591.538 | 0.868× | 0.764–0.954× | win |
| build_growing_and_drop/1024 | SmallVec4 | 1023.242 | 1.430× | 1.207–1.698× | loss |
| build_growing_and_drop/1024 | SmallVec8 | 962.888 | 1.351× | 1.305–1.520× | loss |
| build_growing_and_drop/4 | Vec | 12.498 | 1.000× | 1.000–1.000× | baseline |
| build_growing_and_drop/4 | JackVec | 11.778 | 0.952× | 0.901–0.952× | win |
| build_growing_and_drop/4 | ThinVec | 11.487 | 0.912× | 0.880–0.933× | win |
| build_growing_and_drop/4 | SmallVec4 | 10.892 | 0.868× | 0.835–0.881× | win |
| build_growing_and_drop/4 | SmallVec8 | 8.801 | 0.701× | 0.665–0.789× | win |
| dedup_adjacent_pairs/64_byte | Vec | 973.567 | 1.000× | 1.000–1.000× | baseline |
| dedup_adjacent_pairs/64_byte | JackVec | 1069.736 | 1.099× | 1.096–1.101× | loss |
| dedup_adjacent_pairs/64_byte | ThinVec | 1055.338 | 1.081× | 1.081–1.134× | loss |
| dedup_adjacent_pairs/64_byte | SmallVec4 | 1098.574 | 1.129× | 1.127–1.133× | loss |
| dedup_adjacent_pairs/64_byte | SmallVec8 | 1099.733 | 1.130× | 1.129–1.134× | loss |
| dedup_adjacent_pairs/u64 | Vec | 305.536 | 1.000× | 1.000–1.000× | baseline |
| dedup_adjacent_pairs/u64 | JackVec | 330.173 | 1.082× | 1.063–1.098× | loss |
| dedup_adjacent_pairs/u64 | ThinVec | 440.885 | 1.443× | 1.408–1.739× | loss |
| dedup_adjacent_pairs/u64 | SmallVec4 | 394.484 | 1.284× | 1.271–1.385× | loss |
| dedup_adjacent_pairs/u64 | SmallVec8 | 395.934 | 1.308× | 1.285–1.316× | loss |
| extend_reserved_1024 | Vec | 153.273 | 1.000× | 1.000–1.000× | baseline |
| extend_reserved_1024 | JackVec | 157.306 | 1.026× | 1.014–1.068× | inconclusive |
| extend_reserved_1024 | ThinVec | 149.620 | 0.981× | 0.970–0.987× | inconclusive |
| extend_reserved_1024 | SmallVec4 | 183.859 | 1.200× | 1.189–1.231× | loss |
| extend_reserved_1024 | SmallVec8 | 180.767 | 1.173× | 1.160–1.223× | loss |
| nested_construct_and_drop/empty | Vec | 19014.964 | 1.000× | 1.000–1.000× | baseline |
| nested_construct_and_drop/empty | JackVec | 10728.954 | 0.562× | 0.502–0.571× | win |
| nested_construct_and_drop/empty | ThinVec | 10747.945 | 0.565× | 0.563–0.579× | win |
| nested_construct_and_drop/empty | SmallVec4 | 22329.323 | 1.185× | 1.171–1.207× | loss |
| nested_construct_and_drop/empty | SmallVec8 | 27415.132 | 1.464× | 1.430–1.476× | loss |
| nested_construct_and_drop/small | Vec | 229960.673 | 1.000× | 1.000–1.000× | baseline |
| nested_construct_and_drop/small | JackVec | 211679.807 | 0.921× | 0.902–0.950× | win |
| nested_construct_and_drop/small | ThinVec | 213783.821 | 0.933× | 0.898–0.944× | win |
| nested_construct_and_drop/small | SmallVec4 | 128571.816 | 0.560× | 0.557–0.568× | win |
| nested_construct_and_drop/small | SmallVec8 | 132941.993 | 0.578× | 0.572–0.591× | win |
| nested_construct_and_drop/sparse | Vec | 63301.274 | 1.000× | 1.000–1.000× | baseline |
| nested_construct_and_drop/sparse | JackVec | 56520.544 | 0.889× | 0.873–0.901× | win |
| nested_construct_and_drop/sparse | ThinVec | 53792.593 | 0.853× | 0.836–0.858× | win |
| nested_construct_and_drop/sparse | SmallVec4 | 40854.361 | 0.644× | 0.639–0.650× | win |
| nested_construct_and_drop/sparse | SmallVec8 | 45760.691 | 0.722× | 0.716–0.726× | win |
| nested_metadata_scan_sparse | Vec | 3973.950 | 1.000× | 1.000–1.000× | baseline |
| nested_metadata_scan_sparse | JackVec | 4003.633 | 1.007× | 1.006–1.008× | equivalent |
| nested_metadata_scan_sparse | ThinVec | 4002.153 | 1.007× | 1.006–1.009× | equivalent |
| nested_metadata_scan_sparse | SmallVec4 | 4192.713 | 1.054× | 1.016–1.061× | inconclusive |
| nested_metadata_scan_sparse | SmallVec8 | 4933.846 | 1.242× | 1.234–1.340× | loss |
| nested_traverse/empty | Vec | 2191.791 | 1.000× | 1.000–1.000× | baseline |
| nested_traverse/empty | JackVec | 2152.581 | 0.982× | 0.981–0.982× | equivalent |
| nested_traverse/empty | ThinVec | 2152.570 | 0.982× | 0.982–0.984× | equivalent |
| nested_traverse/empty | SmallVec4 | 5880.295 | 2.683× | 2.680–2.691× | loss |
| nested_traverse/empty | SmallVec8 | 6064.435 | 2.767× | 2.767–2.818× | loss |
| nested_traverse/small | Vec | 21424.143 | 1.000× | 1.000–1.000× | baseline |
| nested_traverse/small | JackVec | 21417.230 | 0.999× | 0.999–1.000× | equivalent |
| nested_traverse/small | ThinVec | 21425.859 | 1.000× | 0.998–1.003× | equivalent |
| nested_traverse/small | SmallVec4 | 21408.585 | 0.999× | 0.995–1.000× | equivalent |
| nested_traverse/small | SmallVec8 | 21425.626 | 1.000× | 0.995–1.007× | equivalent |
| nested_traverse/sparse | Vec | 3081.513 | 1.000× | 1.000–1.000× | baseline |
| nested_traverse/sparse | JackVec | 3151.406 | 1.017× | 1.003–1.140× | inconclusive |
| nested_traverse/sparse | ThinVec | 3117.123 | 1.014× | 1.000–1.065× | inconclusive |
| nested_traverse/sparse | SmallVec4 | 6885.922 | 2.241× | 2.192–2.256× | loss |
| nested_traverse/sparse | SmallVec8 | 5397.225 | 1.768× | 1.742–1.836× | loss |
| push_preallocated/1024 | Vec | 447.832 | 1.000× | 1.000–1.000× | baseline |
| push_preallocated/1024 | JackVec | 427.491 | 0.954× | 0.951–0.970× | inconclusive |
| push_preallocated/1024 | ThinVec | 421.487 | 0.941× | 0.932–0.947× | win |
| push_preallocated/1024 | SmallVec4 | 719.885 | 1.607× | 1.591–1.615× | loss |
| push_preallocated/1024 | SmallVec8 | 724.550 | 1.618× | 1.603–1.647× | loss |
| push_preallocated/4 | Vec | 2.426 | 1.000× | 1.000–1.000× | baseline |
| push_preallocated/4 | JackVec | 2.150 | 0.883× | 0.873–0.893× | win |
| push_preallocated/4 | ThinVec | 2.245 | 0.932× | 0.922–0.934× | win |
| push_preallocated/4 | SmallVec4 | 14.131 | 5.820× | 5.738–5.889× | loss |
| push_preallocated/4 | SmallVec8 | 14.142 | 5.828× | 5.727–5.893× | loss |
| resize_reserved_1024 | Vec | 143.972 | 1.000× | 1.000–1.000× | baseline |
| resize_reserved_1024 | JackVec | 152.352 | 1.058× | 1.037–1.062× | loss |
| resize_reserved_1024 | ThinVec | 430.522 | 2.993× | 2.889–3.014× | loss |
| resize_reserved_1024 | SmallVec4 | 144.410 | 1.001× | 0.974–1.019× | equivalent |
| resize_reserved_1024 | SmallVec8 | 142.411 | 0.991× | 0.936–1.002× | inconclusive |
| retain_mixed/64_byte | Vec | 289.000 | 1.000× | 1.000–1.000× | baseline |
| retain_mixed/64_byte | JackVec | 319.478 | 1.113× | 1.091–1.116× | loss |
| retain_mixed/64_byte | ThinVec | 450.623 | 1.563× | 1.551–1.572× | loss |
| retain_mixed/64_byte | SmallVec4 | 549.839 | 1.900× | 1.884–1.924× | loss |
| retain_mixed/64_byte | SmallVec8 | 545.018 | 1.885× | 1.870–1.907× | loss |
| retain_mixed/u64 | Vec | 563.791 | 1.000× | 1.000–1.000× | baseline |
| retain_mixed/u64 | JackVec | 697.884 | 1.234× | 1.233–1.247× | loss |
| retain_mixed/u64 | ThinVec | 715.956 | 1.270× | 1.264–1.286× | loss |
| retain_mixed/u64 | SmallVec4 | 923.825 | 1.648× | 1.630–1.653× | loss |
| retain_mixed/u64 | SmallVec8 | 926.834 | 1.643× | 1.634–1.659× | loss |
| sequential_iteration/1024 | Vec | 102.530 | 1.000× | 1.000–1.000× | baseline |
| sequential_iteration/1024 | JackVec | 102.593 | 1.002× | 0.999–1.005× | equivalent |
| sequential_iteration/1024 | ThinVec | 102.545 | 1.000× | 0.997–1.002× | equivalent |
| sequential_iteration/1024 | SmallVec4 | 102.525 | 1.000× | 0.997–1.000× | equivalent |
| sequential_iteration/1024 | SmallVec8 | 102.515 | 1.000× | 0.997–1.000× | equivalent |
| sequential_iteration/8 | Vec | 1.814 | 1.000× | 1.000–1.000× | baseline |
| sequential_iteration/8 | JackVec | 1.656 | 0.914× | 0.909–0.914× | win |
| sequential_iteration/8 | ThinVec | 1.818 | 1.003× | 0.997–1.016× | equivalent |
| sequential_iteration/8 | SmallVec4 | 1.654 | 0.912× | 0.906–0.914× | win |
| sequential_iteration/8 | SmallVec8 | 1.661 | 0.916× | 0.909–1.003× | inconclusive |

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
