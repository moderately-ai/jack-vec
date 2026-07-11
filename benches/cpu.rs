mod support;

use std::hint::black_box;

use criterion::measurement::WallTime;
use criterion::{
    criterion_group, criterion_main, BatchSize, BenchmarkGroup, BenchmarkId, Criterion, Throughput,
};
use jackvec::JackVec;

use support::{
    build_growing, build_nested, build_reserved, fill_vector, sum_nested, sum_vector, BenchVector,
    NestedWorkload, APPEND_SIZES, ITERATION_SIZES, NESTED_VECTOR_COUNT, OPERATION_SIZES,
};

fn bench_nested_construct<V: BenchVector<u64>>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    workload: NestedWorkload,
) {
    group.bench_function(BenchmarkId::new(workload.label(), V::LABEL), |bencher| {
        bencher.iter(|| {
            black_box(build_nested::<V>(workload, NESTED_VECTOR_COUNT));
        });
    });
}

fn nested_construct(c: &mut Criterion) {
    let mut group = c.benchmark_group("nested_construct_and_drop");
    group.throughput(Throughput::Elements(NESTED_VECTOR_COUNT as u64));

    for workload in NestedWorkload::ALL {
        bench_nested_construct::<Vec<u64>>(&mut group, workload);
        bench_nested_construct::<JackVec<u64>>(&mut group, workload);
    }

    group.finish();
}

fn bench_nested_traverse<V: BenchVector<u64>>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    workload: NestedWorkload,
) {
    let values = build_nested::<V>(workload, NESTED_VECTOR_COUNT);
    group.bench_function(BenchmarkId::new(workload.label(), V::LABEL), |bencher| {
        bencher.iter(|| black_box(sum_nested(black_box(&values))))
    });
}

fn nested_traverse(c: &mut Criterion) {
    let mut group = c.benchmark_group("nested_traverse");
    group.throughput(Throughput::Elements(NESTED_VECTOR_COUNT as u64));

    for workload in NestedWorkload::ALL {
        bench_nested_traverse::<Vec<u64>>(&mut group, workload);
        bench_nested_traverse::<JackVec<u64>>(&mut group, workload);
    }

    group.finish();
}

fn bench_build<V, F>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    implementation: &'static str,
    len: usize,
    build: F,
) where
    V: BenchVector<u64>,
    F: Fn(usize) -> V,
{
    group.bench_function(BenchmarkId::new(implementation, len), |bencher| {
        bencher.iter(|| black_box(build(black_box(len))));
    });
}

fn build_growing_and_drop(c: &mut Criterion) {
    let mut group = c.benchmark_group("build_growing_and_drop");

    for &len in OPERATION_SIZES {
        group.throughput(Throughput::Elements(len as u64));
        bench_build::<Vec<u64>, _>(&mut group, "Vec", len, build_growing::<Vec<u64>>);
        bench_build::<JackVec<u64>, _>(&mut group, "JackVec", len, build_growing::<JackVec<u64>>);
    }

    group.finish();
}

fn bench_push_preallocated<V: BenchVector<u64>>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    len: usize,
) {
    group.bench_function(BenchmarkId::new(V::LABEL, len), |bencher| {
        // The allocation belongs to setup and destruction happens after the
        // measurement, leaving only element initialization and push overhead
        // in the timed routine.
        bencher.iter_batched_ref(
            || V::with_capacity(len),
            |values| {
                fill_vector(values, black_box(len), 0);
                black_box(values);
            },
            BatchSize::SmallInput,
        );
    });
}

fn push_preallocated(c: &mut Criterion) {
    let mut group = c.benchmark_group("push_preallocated");

    for &len in OPERATION_SIZES {
        group.throughput(Throughput::Elements(len as u64));
        bench_push_preallocated::<Vec<u64>>(&mut group, len);
        bench_push_preallocated::<JackVec<u64>>(&mut group, len);
    }

    group.finish();
}

fn bench_iteration<V: BenchVector<u64>>(group: &mut BenchmarkGroup<'_, WallTime>, len: usize) {
    let values = build_reserved::<V>(len);
    group.bench_function(BenchmarkId::new(V::LABEL, len), |bencher| {
        bencher.iter(|| black_box(sum_vector(black_box(&values))));
    });
}

fn sequential_iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("sequential_iteration");

    for &len in ITERATION_SIZES {
        group.throughput(Throughput::Elements(len as u64));
        bench_iteration::<Vec<u64>>(&mut group, len);
        bench_iteration::<JackVec<u64>>(&mut group, len);
    }

    group.finish();
}

fn bench_append<V: BenchVector<u64>>(group: &mut BenchmarkGroup<'_, WallTime>, len: usize) {
    group.bench_function(BenchmarkId::new(V::LABEL, len), |bencher| {
        bencher.iter_batched_ref(
            || {
                let mut destination = V::with_capacity(len * 2);
                fill_vector(&mut destination, len, 0);
                let source = build_reserved::<V>(len);
                (destination, source)
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
        bench_append::<Vec<u64>>(&mut group, len);
        bench_append::<JackVec<u64>>(&mut group, len);
    }

    group.finish();
}

fn bench_retain_u64<V: BenchVector<u64>>(group: &mut BenchmarkGroup<'_, WallTime>) {
    group.bench_function(BenchmarkId::new("u64", V::LABEL), |bencher| {
        bencher.iter_batched_ref(
            || build_reserved::<V>(1_024),
            |values| {
                values.retain_mut(|value| black_box(*value) % 2 == 0);
                debug_assert_eq!(values.as_slice().len(), 512);
                black_box(values);
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_retain_large<V: BenchVector<[u64; 8]>>(group: &mut BenchmarkGroup<'_, WallTime>) {
    group.bench_function(BenchmarkId::new("64_byte", V::LABEL), |bencher| {
        bencher.iter_batched_ref(
            || {
                let mut values = V::with_capacity(256);
                for index in 0..256 {
                    values.push([index; 8]);
                }
                values
            },
            |values| {
                values.retain_mut(|value| black_box(value[0]) % 2 == 0);
                debug_assert_eq!(values.as_slice().len(), 128);
                black_box(values);
            },
            BatchSize::SmallInput,
        );
    });
}

fn retain_mixed(c: &mut Criterion) {
    let mut group = c.benchmark_group("retain_mixed");

    group.throughput(Throughput::Elements(1_024));
    bench_retain_u64::<Vec<u64>>(&mut group);
    bench_retain_u64::<JackVec<u64>>(&mut group);

    group.throughput(Throughput::Elements(256));
    bench_retain_large::<Vec<[u64; 8]>>(&mut group);
    bench_retain_large::<JackVec<[u64; 8]>>(&mut group);

    group.finish();
}

fn bench_dedup_u64<V: BenchVector<u64>>(group: &mut BenchmarkGroup<'_, WallTime>) {
    group.bench_function(BenchmarkId::new("u64", V::LABEL), |bencher| {
        bencher.iter_batched_ref(
            || {
                let mut values = V::with_capacity(1_024);
                for index in 0..1_024 {
                    values.push(index / 2);
                }
                values
            },
            |values| {
                values.dedup_by(|current, previous| black_box(*current) == *previous);
                debug_assert_eq!(values.as_slice().len(), 512);
                black_box(values);
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_dedup_large<V: BenchVector<[u64; 8]>>(group: &mut BenchmarkGroup<'_, WallTime>) {
    group.bench_function(BenchmarkId::new("64_byte", V::LABEL), |bencher| {
        bencher.iter_batched_ref(
            || {
                let mut values = V::with_capacity(256);
                for index in 0..256 {
                    values.push([index / 2; 8]);
                }
                values
            },
            |values| {
                values.dedup_by(|current, previous| black_box(current[0]) == previous[0]);
                debug_assert_eq!(values.as_slice().len(), 128);
                black_box(values);
            },
            BatchSize::SmallInput,
        );
    });
}

fn dedup_adjacent_pairs(c: &mut Criterion) {
    let mut group = c.benchmark_group("dedup_adjacent_pairs");

    group.throughput(Throughput::Elements(1_024));
    bench_dedup_u64::<Vec<u64>>(&mut group);
    bench_dedup_u64::<JackVec<u64>>(&mut group);

    group.throughput(Throughput::Elements(256));
    bench_dedup_large::<Vec<[u64; 8]>>(&mut group);
    bench_dedup_large::<JackVec<[u64; 8]>>(&mut group);

    group.finish();
}

fn bench_extend_reserved<V: BenchVector<u64>>(group: &mut BenchmarkGroup<'_, WallTime>) {
    const LEN: usize = 1_024;

    group.bench_function(V::LABEL, |bencher| {
        bencher.iter_batched_ref(
            || V::with_capacity(LEN),
            |values| {
                values.extend(0..black_box(LEN as u64));
                debug_assert_eq!(values.as_slice().len(), LEN);
                black_box(values);
            },
            BatchSize::SmallInput,
        );
    });
}

fn extend_reserved(c: &mut Criterion) {
    let mut group = c.benchmark_group("extend_reserved_1024");
    group.throughput(Throughput::Elements(1_024));
    bench_extend_reserved::<Vec<u64>>(&mut group);
    bench_extend_reserved::<JackVec<u64>>(&mut group);
    group.finish();
}

fn bench_resize_reserved<V: BenchVector<u64>>(group: &mut BenchmarkGroup<'_, WallTime>) {
    const LEN: usize = 1_024;

    group.bench_function(V::LABEL, |bencher| {
        bencher.iter_batched_ref(
            || V::with_capacity(LEN),
            |values| {
                values.resize(black_box(LEN), black_box(7));
                debug_assert_eq!(values.as_slice().len(), LEN);
                black_box(values);
            },
            BatchSize::SmallInput,
        );
    });
}

fn resize_reserved(c: &mut Criterion) {
    let mut group = c.benchmark_group("resize_reserved_1024");
    group.throughput(Throughput::Elements(1_024));
    bench_resize_reserved::<Vec<u64>>(&mut group);
    bench_resize_reserved::<JackVec<u64>>(&mut group);
    group.finish();
}

fn jack_into_vec(c: &mut Criterion) {
    let mut group = c.benchmark_group("jack_into_vec");

    for &len in &[4, 1_024] {
        group.throughput(Throughput::Elements(len as u64));
        let source = build_reserved::<JackVec<u64>>(len);
        group.bench_function(BenchmarkId::new("JackVec", len), |bencher| {
            bencher.iter_batched(
                || source.clone(),
                |values| Vec::from(black_box(values)),
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn jack_into_box(c: &mut Criterion) {
    let mut group = c.benchmark_group("jack_into_box");
    let len = 1_024;
    group.throughput(Throughput::Elements(len as u64));
    let source = build_reserved::<JackVec<u64>>(len);
    group.bench_function("JackVec", |bencher| {
        bencher.iter_batched(
            || source.clone(),
            |values| Box::<[u64]>::from(black_box(values)),
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

fn vec_into_jack(c: &mut Criterion) {
    let mut group = c.benchmark_group("vec_into_jack");
    let len = 1_024;
    group.throughput(Throughput::Elements(len as u64));
    let source = build_reserved::<Vec<u64>>(len);
    group.bench_function("Vec", |bencher| {
        bencher.iter_batched(
            || source.clone(),
            |values| JackVec::from(black_box(values)),
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

fn array_into_jack(c: &mut Criterion) {
    let mut group = c.benchmark_group("array_into_jack");
    group.throughput(Throughput::Elements(4));
    group.bench_function("array_4", |bencher| {
        bencher.iter_batched(
            || [0_u64, 1, 2, 3],
            |values| JackVec::from(black_box(values)),
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

criterion_group!(
    benches,
    nested_construct,
    nested_traverse,
    build_growing_and_drop,
    push_preallocated,
    sequential_iteration,
    append_preallocated,
    retain_mixed,
    dedup_adjacent_pairs,
    extend_reserved,
    resize_reserved,
    jack_into_vec,
    jack_into_box,
    vec_into_jack,
    array_into_jack
);
criterion_main!(benches);
