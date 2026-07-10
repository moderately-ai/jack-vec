mod support;

use std::hint::black_box;

use criterion::measurement::WallTime;
use criterion::{
    criterion_group, criterion_main, BatchSize, BenchmarkGroup, BenchmarkId, Criterion, Throughput,
};
use thin_vec::ThinVec;

use support::{
    build_growing, build_nested, build_reserved, fill_vector, sum_nested, sum_vector, BenchVector,
    NestedWorkload, ITERATION_SIZES, NESTED_VECTOR_COUNT, OPERATION_SIZES,
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
        bench_nested_construct::<ThinVec<u64>>(&mut group, workload);
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
        bench_nested_traverse::<ThinVec<u64>>(&mut group, workload);
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
        bench_build::<ThinVec<u64>, _>(&mut group, "ThinVec", len, build_growing::<ThinVec<u64>>);
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
        bench_push_preallocated::<ThinVec<u64>>(&mut group, len);
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
        bench_iteration::<ThinVec<u64>>(&mut group, len);
    }

    group.finish();
}

criterion_group!(
    benches,
    nested_construct,
    nested_traverse,
    build_growing_and_drop,
    push_preallocated,
    sequential_iteration
);
criterion_main!(benches);
