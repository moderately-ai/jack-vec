#[allow(dead_code)]
mod support;

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::mem::size_of;
use std::sync::atomic::{AtomicUsize, Ordering};

use thin_vec::ThinVec;

use support::{
    build_growing, build_nested, build_reserved, BenchVector, NestedWorkload, NESTED_VECTOR_COUNT,
    OPERATION_SIZES,
};

struct TrackingAllocator;

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_BYTES: AtomicUsize = AtomicUsize::new(0);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static REALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static DEALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator;

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: The caller provides the layout required by GlobalAlloc::alloc,
        // and the pointer is returned unchanged.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            add_live_bytes(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: The caller provides the layout required by
        // GlobalAlloc::alloc_zeroed, and the pointer is returned unchanged.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            add_live_bytes(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: The pointer and layout are forwarded unchanged to the
        // allocator that created the allocation.
        unsafe { System.dealloc(pointer, layout) };
        DEALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: The pointer, old layout, and new size are forwarded unchanged
        // and satisfy GlobalAlloc::realloc's contract by delegation.
        let new_pointer = unsafe { System.realloc(pointer, layout, new_size) };
        if !new_pointer.is_null() {
            REALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            if new_size >= layout.size() {
                add_live_bytes(new_size - layout.size());
            } else {
                LIVE_BYTES.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        new_pointer
    }
}

fn add_live_bytes(bytes: usize) {
    let live = LIVE_BYTES.fetch_add(bytes, Ordering::Relaxed) + bytes;
    let mut peak = PEAK_BYTES.load(Ordering::Relaxed);
    while live > peak {
        match PEAK_BYTES.compare_exchange_weak(peak, live, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(observed) => peak = observed,
        }
    }
}

struct Measurement {
    live_before_drop: usize,
    peak_live: usize,
    live_after_drop: usize,
    allocations: usize,
    reallocations: usize,
    deallocations: usize,
}

fn measure<T>(operation: impl FnOnce() -> T) -> Measurement {
    let baseline_live = LIVE_BYTES.load(Ordering::Relaxed);
    PEAK_BYTES.store(baseline_live, Ordering::Relaxed);
    ALLOCATIONS.store(0, Ordering::Relaxed);
    REALLOCATIONS.store(0, Ordering::Relaxed);
    DEALLOCATIONS.store(0, Ordering::Relaxed);

    let value = operation();
    black_box(&value);
    let live_before_drop = LIVE_BYTES.load(Ordering::Relaxed) - baseline_live;
    drop(value);

    Measurement {
        live_before_drop,
        peak_live: PEAK_BYTES.load(Ordering::Relaxed) - baseline_live,
        live_after_drop: LIVE_BYTES.load(Ordering::Relaxed) - baseline_live,
        allocations: ALLOCATIONS.load(Ordering::Relaxed),
        reallocations: REALLOCATIONS.load(Ordering::Relaxed),
        deallocations: DEALLOCATIONS.load(Ordering::Relaxed),
    }
}

fn report<V, T>(
    benchmark: &str,
    input: impl std::fmt::Display,
    container_count: usize,
    operation: T,
) where
    V: BenchVector<u64>,
    T: FnOnce() -> Vec<V>,
{
    let measurement = measure(operation);
    assert_eq!(measurement.live_after_drop, 0, "benchmark workload leaked");
    println!(
        "{},{},{},{},{},{},{},{},{},{},{}",
        benchmark,
        input,
        V::LABEL,
        container_count,
        size_of::<V>(),
        measurement.live_before_drop,
        measurement.peak_live,
        measurement.live_after_drop,
        measurement.allocations,
        measurement.reallocations,
        measurement.deallocations,
    );
}

fn report_vector<V, T>(benchmark: &str, len: usize, operation: T)
where
    V: BenchVector<u64>,
    T: FnOnce() -> V,
{
    let measurement = measure(operation);
    assert_eq!(measurement.live_after_drop, 0, "benchmark workload leaked");
    println!(
        "{},{},{},{},{},{},{},{},{},{},{}",
        benchmark,
        len,
        V::LABEL,
        1,
        size_of::<V>(),
        measurement.live_before_drop,
        measurement.peak_live,
        measurement.live_after_drop,
        measurement.allocations,
        measurement.reallocations,
        measurement.deallocations,
    );
}

fn report_thin_into_vec(len: usize) {
    let measurement = measure(|| {
        let output = Vec::from(build_reserved::<ThinVec<u64>>(len));
        assert_eq!(output.len(), len);
        assert_eq!(output.capacity(), len);
        output
    });
    assert_eq!(measurement.live_after_drop, 0, "benchmark workload leaked");
    println!(
        "{},{},{},{},{},{},{},{},{},{},{}",
        "thin_into_vec",
        len,
        "ThinVec_to_Vec",
        1,
        size_of::<Vec<u64>>(),
        measurement.live_before_drop,
        measurement.peak_live,
        measurement.live_after_drop,
        measurement.allocations,
        measurement.reallocations,
        measurement.deallocations,
    );
}

fn main() {
    println!(
        "benchmark,input,implementation,container_count,inline_bytes,live_requested_bytes,\
         peak_requested_bytes,live_after_drop_bytes,allocations,reallocations,deallocations"
    );

    for workload in NestedWorkload::ALL {
        report::<Vec<u64>, _>("nested", workload.label(), NESTED_VECTOR_COUNT, || {
            build_nested::<Vec<u64>>(workload, NESTED_VECTOR_COUNT)
        });
        report::<ThinVec<u64>, _>("nested", workload.label(), NESTED_VECTOR_COUNT, || {
            build_nested::<ThinVec<u64>>(workload, NESTED_VECTOR_COUNT)
        });
    }

    for &len in OPERATION_SIZES {
        report_vector::<Vec<u64>, _>("build_growing", len, || build_growing::<Vec<u64>>(len));
        report_vector::<ThinVec<u64>, _>("build_growing", len, || {
            build_growing::<ThinVec<u64>>(len)
        });
        report_vector::<Vec<u64>, _>("push_reserved", len, || build_reserved::<Vec<u64>>(len));
        report_vector::<ThinVec<u64>, _>("push_reserved", len, || {
            build_reserved::<ThinVec<u64>>(len)
        });
    }

    for &len in &[4, 1_024] {
        report_thin_into_vec(len);
    }
}
