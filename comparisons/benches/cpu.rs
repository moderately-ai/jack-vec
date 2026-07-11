use std::hint::black_box;
use std::time::Duration;

use criterion::measurement::WallTime;
use criterion::{
    criterion_group, criterion_main, BatchSize, BenchmarkGroup, BenchmarkId, Criterion, Throughput,
};
use jack_vec::JackVec;
use jack_vec_comparisons::{
    build_growing, build_nested, build_reserved, fill_vector, metadata_checksum, sum_nested,
    BenchVector, NestedWorkload, SmallVec4, SmallVec8, APPEND_SIZES, GROWING_SIZES,
    ITERATION_SIZES, NESTED_VECTOR_COUNT, RESERVED_PUSH_SIZES,
};
use thin_vec::ThinVec;

macro_rules! every_vector {
    ($function:ident, $group:expr $(, $argument:expr)*) => {{
        match rotation() {
            0 => {
                $function::<Vec<u64>>($group $(, $argument)*);
                $function::<JackVec<u64>>($group $(, $argument)*);
                $function::<ThinVec<u64>>($group $(, $argument)*);
                $function::<SmallVec4<u64>>($group $(, $argument)*);
                $function::<SmallVec8<u64>>($group $(, $argument)*);
            }
            1 => {
                $function::<JackVec<u64>>($group $(, $argument)*);
                $function::<ThinVec<u64>>($group $(, $argument)*);
                $function::<SmallVec4<u64>>($group $(, $argument)*);
                $function::<SmallVec8<u64>>($group $(, $argument)*);
                $function::<Vec<u64>>($group $(, $argument)*);
            }
            2 => {
                $function::<ThinVec<u64>>($group $(, $argument)*);
                $function::<SmallVec4<u64>>($group $(, $argument)*);
                $function::<SmallVec8<u64>>($group $(, $argument)*);
                $function::<Vec<u64>>($group $(, $argument)*);
                $function::<JackVec<u64>>($group $(, $argument)*);
            }
            3 => {
                $function::<SmallVec4<u64>>($group $(, $argument)*);
                $function::<SmallVec8<u64>>($group $(, $argument)*);
                $function::<Vec<u64>>($group $(, $argument)*);
                $function::<JackVec<u64>>($group $(, $argument)*);
                $function::<ThinVec<u64>>($group $(, $argument)*);
            }
            _ => {
                $function::<SmallVec8<u64>>($group $(, $argument)*);
                $function::<Vec<u64>>($group $(, $argument)*);
                $function::<JackVec<u64>>($group $(, $argument)*);
                $function::<ThinVec<u64>>($group $(, $argument)*);
                $function::<SmallVec4<u64>>($group $(, $argument)*);
            }
        }
    }};
}

fn rotation() -> usize {
    std::env::var("JACK_VEC_BENCH_ROTATION")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
        % 5
}

macro_rules! every_typed_vector {
    ($function:ident, $element:ty, $group:expr $(, $argument:expr)*) => {{
        macro_rules! call {
            (Vec) => { $function::<$element, Vec<$element>, _>($group $(, $argument)*) };
            (JackVec) => { $function::<$element, JackVec<$element>, _>($group $(, $argument)*) };
            (ThinVec) => { $function::<$element, ThinVec<$element>, _>($group $(, $argument)*) };
            (SmallVec4) => { $function::<$element, SmallVec4<$element>, _>($group $(, $argument)*) };
            (SmallVec8) => { $function::<$element, SmallVec8<$element>, _>($group $(, $argument)*) };
        }
        match rotation() {
            0 => { call!(Vec); call!(JackVec); call!(ThinVec); call!(SmallVec4); call!(SmallVec8); }
            1 => { call!(JackVec); call!(ThinVec); call!(SmallVec4); call!(SmallVec8); call!(Vec); }
            2 => { call!(ThinVec); call!(SmallVec4); call!(SmallVec8); call!(Vec); call!(JackVec); }
            3 => { call!(SmallVec4); call!(SmallVec8); call!(Vec); call!(JackVec); call!(ThinVec); }
            _ => { call!(SmallVec8); call!(Vec); call!(JackVec); call!(ThinVec); call!(SmallVec4); }
        }
    }};
}

fn bench_nested_construct<V: BenchVector<u64>>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    workload: NestedWorkload,
) {
    group.bench_function(BenchmarkId::new(workload.label(), V::LABEL), |bencher| {
        bencher.iter(|| black_box(build_nested::<V>(workload, NESTED_VECTOR_COUNT)));
    });
}

fn nested_construct(c: &mut Criterion) {
    let mut group = c.benchmark_group("nested_construct_and_drop");
    group.throughput(Throughput::Elements(NESTED_VECTOR_COUNT as u64));
    for workload in NestedWorkload::ALL {
        every_vector!(bench_nested_construct, &mut group, workload);
    }
    group.finish();
}

fn bench_nested_traverse<V: BenchVector<u64>>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    workload: NestedWorkload,
) {
    let values = build_nested::<V>(workload, NESTED_VECTOR_COUNT);
    group.bench_function(BenchmarkId::new(workload.label(), V::LABEL), |bencher| {
        bencher.iter(|| black_box(sum_nested(black_box(&values))));
    });
}

fn nested_traverse(c: &mut Criterion) {
    let mut group = c.benchmark_group("nested_traverse");
    group.throughput(Throughput::Elements(NESTED_VECTOR_COUNT as u64));
    for workload in NestedWorkload::ALL {
        every_vector!(bench_nested_traverse, &mut group, workload);
    }
    group.finish();
}

fn bench_metadata<V: BenchVector<u64>>(group: &mut BenchmarkGroup<'_, WallTime>) {
    let values = build_nested::<V>(NestedWorkload::Sparse, NESTED_VECTOR_COUNT);
    group.bench_function(V::LABEL, |bencher| {
        bencher.iter(|| black_box(metadata_checksum(black_box(&values))));
    });
}

fn nested_metadata_scan(c: &mut Criterion) {
    let mut group = c.benchmark_group("nested_metadata_scan_sparse");
    group.throughput(Throughput::Elements(NESTED_VECTOR_COUNT as u64));
    every_vector!(bench_metadata, &mut group);
    group.finish();
}

fn bench_build_growing<V: BenchVector<u64>>(group: &mut BenchmarkGroup<'_, WallTime>, len: usize) {
    group.bench_function(BenchmarkId::new(V::LABEL, len), |bencher| {
        bencher.iter(|| black_box(build_growing::<V>(black_box(len))));
    });
}

fn build_growing_and_drop(c: &mut Criterion) {
    let mut group = c.benchmark_group("build_growing_and_drop");
    for &len in GROWING_SIZES {
        group.throughput(Throughput::Elements(len as u64));
        every_vector!(bench_build_growing, &mut group, len);
    }
    group.finish();
}

fn bench_push_reserved<V: BenchVector<u64>>(group: &mut BenchmarkGroup<'_, WallTime>, len: usize) {
    group.bench_function(BenchmarkId::new(V::LABEL, len), |bencher| {
        bencher.iter_batched_ref(
            || V::with_capacity(len),
            |values| {
                fill_vector(values, black_box(len), |index| index as u64);
                black_box(values);
            },
            BatchSize::SmallInput,
        );
    });
}

fn push_preallocated(c: &mut Criterion) {
    let mut group = c.benchmark_group("push_preallocated");
    for &len in RESERVED_PUSH_SIZES {
        group.throughput(Throughput::Elements(len as u64));
        every_vector!(bench_push_reserved, &mut group, len);
    }
    group.finish();
}

fn bench_iteration<V: BenchVector<u64>>(group: &mut BenchmarkGroup<'_, WallTime>, len: usize) {
    let values = build_reserved::<V>(len);
    group.bench_function(BenchmarkId::new(V::LABEL, len), |bencher| {
        bencher.iter(|| {
            black_box(
                values
                    .as_slice()
                    .iter()
                    .fold(0_u64, |sum, value| sum.wrapping_add(*value)),
            )
        });
    });
}

fn sequential_iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("sequential_iteration");
    for &len in ITERATION_SIZES {
        group.throughput(Throughput::Elements(len as u64));
        every_vector!(bench_iteration, &mut group, len);
    }
    group.finish();
}

fn bench_append<V: BenchVector<u64>>(group: &mut BenchmarkGroup<'_, WallTime>, len: usize) {
    group.bench_function(BenchmarkId::new(V::LABEL, len), |bencher| {
        bencher.iter_batched_ref(
            || {
                let mut destination = V::with_capacity(len * 2);
                fill_vector(&mut destination, len, |index| index as u64);
                (destination, build_reserved::<V>(len))
            },
            |(destination, source)| {
                destination.append(source);
                black_box(destination);
            },
            BatchSize::SmallInput,
        );
    });
}

fn append_preallocated(c: &mut Criterion) {
    let mut group = c.benchmark_group("append_preallocated");
    for &len in APPEND_SIZES {
        group.throughput(Throughput::Elements(len as u64));
        every_vector!(bench_append, &mut group, len);
    }
    group.finish();
}

#[derive(Clone, Copy, PartialEq)]
struct LargeElement([u64; 8]);

fn bench_retain<T, V, F>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    label: &'static str,
    len: usize,
    make: F,
) where
    T: Clone,
    V: BenchVector<T> + Clone,
    F: Fn(usize) -> T + Copy,
{
    let mut source = V::with_capacity(len);
    fill_vector(&mut source, len, make);
    group.bench_function(BenchmarkId::new(label, V::LABEL), |bencher| {
        bencher.iter_batched(
            || source.clone(),
            |mut values| {
                let mut index = 0_usize;
                values.retain_mut(|_| {
                    let keep = index % 3 != 0;
                    index += 1;
                    keep
                });
                black_box(values);
            },
            BatchSize::SmallInput,
        );
    });
}

fn retain_mixed(c: &mut Criterion) {
    let mut group = c.benchmark_group("retain_mixed");
    every_typed_vector!(bench_retain, u64, &mut group, "u64", 1_024, |index| index
        as u64);
    every_typed_vector!(
        bench_retain,
        LargeElement,
        &mut group,
        "64_byte",
        256,
        |index| LargeElement([index as u64; 8])
    );
    group.finish();
}

fn bench_dedup<T, V, F>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    label: &'static str,
    len: usize,
    make: F,
) where
    T: Clone + PartialEq,
    V: BenchVector<T> + Clone,
    F: Fn(usize) -> T,
{
    let mut source = V::with_capacity(len);
    fill_vector(&mut source, len, |index| make(index / 2));
    group.bench_function(BenchmarkId::new(label, V::LABEL), |bencher| {
        bencher.iter_batched(
            || source.clone(),
            |mut values| {
                values.dedup_by(|left, right| left == right);
                black_box(values);
            },
            BatchSize::SmallInput,
        );
    });
}

fn dedup_adjacent_pairs(c: &mut Criterion) {
    let mut group = c.benchmark_group("dedup_adjacent_pairs");
    every_typed_vector!(bench_dedup, u64, &mut group, "u64", 1_024, |index| index
        as u64);
    every_typed_vector!(
        bench_dedup,
        LargeElement,
        &mut group,
        "64_byte",
        256,
        |index| LargeElement([index as u64; 8])
    );
    group.finish();
}

fn bench_extend<V: BenchVector<u64>>(group: &mut BenchmarkGroup<'_, WallTime>) {
    group.bench_function(V::LABEL, |bencher| {
        bencher.iter_batched(
            || V::with_capacity(1_024),
            |mut values| {
                values.extend(0_u64..1_024);
                black_box(values);
            },
            BatchSize::SmallInput,
        );
    });
}

fn extend_reserved(c: &mut Criterion) {
    let mut group = c.benchmark_group("extend_reserved_1024");
    group.throughput(Throughput::Elements(1_024));
    every_vector!(bench_extend, &mut group);
    group.finish();
}

fn bench_resize<V: BenchVector<u64>>(group: &mut BenchmarkGroup<'_, WallTime>) {
    group.bench_function(V::LABEL, |bencher| {
        bencher.iter_batched(
            || V::with_capacity(1_024),
            |mut values| {
                values.resize(1_024, black_box(7));
                black_box(values);
            },
            BatchSize::SmallInput,
        );
    });
}

fn resize_reserved(c: &mut Criterion) {
    let mut group = c.benchmark_group("resize_reserved_1024");
    group.throughput(Throughput::Elements(1_024));
    every_vector!(bench_resize, &mut group);
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2))
        .sample_size(50);
    targets = nested_construct, nested_traverse, nested_metadata_scan,
        build_growing_and_drop, push_preallocated, sequential_iteration,
        append_preallocated, retain_mixed, dedup_adjacent_pairs,
        extend_reserved, resize_reserved
}
criterion_main!(benches);
