#[allow(dead_code)]
mod support;

use std::alloc::{GlobalAlloc, Layout, System};
#[cfg(any(target_os = "macos", all(target_os = "linux", target_env = "gnu")))]
use std::ffi::c_void;
use std::hint::black_box;
use std::mem::size_of;
use std::sync::atomic::{AtomicUsize, Ordering};

use jack_vec::JackVec;

use support::{
    build_growing, build_nested, build_reserved, BenchVector, NestedWorkload, NESTED_VECTOR_COUNT,
    OPERATION_SIZES,
};

struct TrackingAllocator;

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE_USABLE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_USABLE_BYTES: AtomicUsize = AtomicUsize::new(0);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static REALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static MOVED_REALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static IN_PLACE_REALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static DEALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn malloc_size(pointer: *const c_void) -> usize;
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
unsafe extern "C" {
    fn malloc_usable_size(pointer: *mut c_void) -> usize;
}

#[cfg(target_os = "macos")]
const USABLE_SIZE_SOURCE: &str = "malloc_size";
#[cfg(all(target_os = "linux", target_env = "gnu"))]
const USABLE_SIZE_SOURCE: &str = "malloc_usable_size";
#[cfg(not(any(target_os = "macos", all(target_os = "linux", target_env = "gnu"))))]
const USABLE_SIZE_SOURCE: &str = "requested_fallback";

fn usable_size(pointer: *mut u8, _requested: usize) -> usize {
    #[cfg(target_os = "macos")]
    unsafe {
        malloc_size(pointer.cast())
    }
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    unsafe {
        malloc_usable_size(pointer.cast())
    }
    #[cfg(not(any(target_os = "macos", all(target_os = "linux", target_env = "gnu"))))]
    {
        let _ = pointer;
        _requested
    }
}

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
            add_live_usable_bytes(usable_size(pointer, layout.size()));
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
            add_live_usable_bytes(usable_size(pointer, layout.size()));
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        let usable = usable_size(pointer, layout.size());
        // SAFETY: The pointer and layout are forwarded unchanged to the
        // allocator that created the allocation.
        unsafe { System.dealloc(pointer, layout) };
        DEALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        LIVE_USABLE_BYTES.fetch_sub(usable, Ordering::Relaxed);
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let old_usable = usable_size(pointer, layout.size());
        // SAFETY: The pointer, old layout, and new size are forwarded unchanged
        // and satisfy GlobalAlloc::realloc's contract by delegation.
        let new_pointer = unsafe { System.realloc(pointer, layout, new_size) };
        if !new_pointer.is_null() {
            REALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            if new_pointer == pointer {
                IN_PLACE_REALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            } else {
                MOVED_REALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            }
            if new_size >= layout.size() {
                add_live_bytes(new_size - layout.size());
            } else {
                LIVE_BYTES.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
            let new_usable = usable_size(new_pointer, new_size);
            if new_usable >= old_usable {
                add_live_usable_bytes(new_usable - old_usable);
            } else {
                LIVE_USABLE_BYTES.fetch_sub(old_usable - new_usable, Ordering::Relaxed);
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

fn add_live_usable_bytes(bytes: usize) {
    let live = LIVE_USABLE_BYTES.fetch_add(bytes, Ordering::Relaxed) + bytes;
    let mut peak = PEAK_USABLE_BYTES.load(Ordering::Relaxed);
    while live > peak {
        match PEAK_USABLE_BYTES.compare_exchange_weak(
            peak,
            live,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(observed) => peak = observed,
        }
    }
}

struct Measurement {
    live_before_drop: usize,
    peak_live: usize,
    live_after_drop: usize,
    usable_before_drop: usize,
    peak_usable: usize,
    usable_after_drop: usize,
    allocations: usize,
    reallocations: usize,
    moved_reallocations: usize,
    in_place_reallocations: usize,
    deallocations: usize,
}

fn measure<T>(operation: impl FnOnce() -> T) -> Measurement {
    let baseline_live = LIVE_BYTES.load(Ordering::Relaxed);
    PEAK_BYTES.store(baseline_live, Ordering::Relaxed);
    let baseline_usable = LIVE_USABLE_BYTES.load(Ordering::Relaxed);
    PEAK_USABLE_BYTES.store(baseline_usable, Ordering::Relaxed);
    ALLOCATIONS.store(0, Ordering::Relaxed);
    REALLOCATIONS.store(0, Ordering::Relaxed);
    MOVED_REALLOCATIONS.store(0, Ordering::Relaxed);
    IN_PLACE_REALLOCATIONS.store(0, Ordering::Relaxed);
    DEALLOCATIONS.store(0, Ordering::Relaxed);

    let value = operation();
    black_box(&value);
    let live_before_drop = LIVE_BYTES.load(Ordering::Relaxed) - baseline_live;
    let usable_before_drop = LIVE_USABLE_BYTES.load(Ordering::Relaxed) - baseline_usable;
    drop(value);

    Measurement {
        live_before_drop,
        peak_live: PEAK_BYTES.load(Ordering::Relaxed) - baseline_live,
        live_after_drop: LIVE_BYTES.load(Ordering::Relaxed) - baseline_live,
        usable_before_drop,
        peak_usable: PEAK_USABLE_BYTES.load(Ordering::Relaxed) - baseline_usable,
        usable_after_drop: LIVE_USABLE_BYTES.load(Ordering::Relaxed) - baseline_usable,
        allocations: ALLOCATIONS.load(Ordering::Relaxed),
        reallocations: REALLOCATIONS.load(Ordering::Relaxed),
        moved_reallocations: MOVED_REALLOCATIONS.load(Ordering::Relaxed),
        in_place_reallocations: IN_PLACE_REALLOCATIONS.load(Ordering::Relaxed),
        deallocations: DEALLOCATIONS.load(Ordering::Relaxed),
    }
}

fn print_measurement(
    benchmark: &str,
    input: impl std::fmt::Display,
    implementation: &str,
    container_count: usize,
    inline_bytes: usize,
    measurement: &Measurement,
) {
    assert_eq!(measurement.live_after_drop, 0, "benchmark workload leaked");
    assert_eq!(
        measurement.usable_after_drop, 0,
        "benchmark workload leaked allocator-usable bytes"
    );
    assert!(measurement.usable_before_drop >= measurement.live_before_drop);
    assert!(measurement.peak_usable >= measurement.peak_live);
    assert_eq!(
        measurement.moved_reallocations + measurement.in_place_reallocations,
        measurement.reallocations
    );
    println!(
        "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
        benchmark,
        input,
        implementation,
        USABLE_SIZE_SOURCE,
        container_count,
        inline_bytes,
        measurement.live_before_drop,
        measurement.peak_live,
        measurement.live_after_drop,
        measurement.usable_before_drop,
        measurement.peak_usable,
        measurement.usable_after_drop,
        measurement.allocations,
        measurement.reallocations,
        measurement.moved_reallocations,
        measurement.in_place_reallocations,
        measurement.deallocations,
    );
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
    print_measurement(
        benchmark,
        input,
        V::LABEL,
        container_count,
        size_of::<V>(),
        &measurement,
    );
}

fn report_vector<V, T>(benchmark: &str, len: usize, operation: T)
where
    V: BenchVector<u64>,
    T: FnOnce() -> V,
{
    let measurement = measure(operation);
    print_measurement(benchmark, len, V::LABEL, 1, size_of::<V>(), &measurement);
}

fn report_reserved_u8(len: usize) {
    let measurement = measure(|| {
        let mut values = JackVec::<u8>::with_capacity(len);
        values.resize(len, 0);
        values
    });
    print_measurement(
        "push_reserved_u8",
        len,
        "JackVec<u8>",
        1,
        size_of::<JackVec<u8>>(),
        &measurement,
    );
}

fn report_jack_into_vec(len: usize) {
    let measurement = measure(|| {
        let output = Vec::from(build_reserved::<JackVec<u64>>(len));
        assert_eq!(output.len(), len);
        assert_eq!(output.capacity(), len);
        output
    });
    print_measurement(
        "jack_into_vec",
        len,
        "JackVec_to_Vec",
        1,
        size_of::<Vec<u64>>(),
        &measurement,
    );
}

fn report_jack_into_box() {
    let len = 1_024;
    let measurement = measure(|| {
        let output = Box::<[u64]>::from(build_reserved::<JackVec<u64>>(len));
        assert_eq!(output.len(), len);
        output
    });
    print_measurement(
        "jack_into_box",
        len,
        "JackVec_to_Box",
        1,
        size_of::<Box<[u64]>>(),
        &measurement,
    );
}

fn report_vec_into_jack() {
    let len = 1_024;
    let measurement = measure(|| {
        let output = JackVec::from(build_reserved::<Vec<u64>>(len));
        assert_eq!(output.len(), len);
        assert_eq!(output.capacity(), len);
        output
    });
    print_measurement(
        "vec_into_jack",
        len,
        "Vec_to_JackVec",
        1,
        size_of::<JackVec<u64>>(),
        &measurement,
    );
}

fn main() {
    println!(
        "benchmark,input,implementation,usable_size_source,container_count,inline_bytes,\
         live_requested_bytes,peak_requested_bytes,live_after_drop_bytes,live_usable_bytes,\
         peak_usable_bytes,usable_after_drop_bytes,allocations,reallocations,\
         moved_reallocations,in_place_reallocations,deallocations"
    );

    for workload in NestedWorkload::ALL {
        report::<Vec<u64>, _>("nested", workload.label(), NESTED_VECTOR_COUNT, || {
            build_nested::<Vec<u64>>(workload, NESTED_VECTOR_COUNT)
        });
        report::<JackVec<u64>, _>("nested", workload.label(), NESTED_VECTOR_COUNT, || {
            build_nested::<JackVec<u64>>(workload, NESTED_VECTOR_COUNT)
        });
    }

    for &len in OPERATION_SIZES {
        report_vector::<Vec<u64>, _>("build_growing", len, || build_growing::<Vec<u64>>(len));
        report_vector::<JackVec<u64>, _>("build_growing", len, || {
            build_growing::<JackVec<u64>>(len)
        });
        report_vector::<Vec<u64>, _>("push_reserved", len, || build_reserved::<Vec<u64>>(len));
        report_vector::<JackVec<u64>, _>("push_reserved", len, || {
            build_reserved::<JackVec<u64>>(len)
        });
        report_reserved_u8(len);
    }

    for &len in &[4, 1_024] {
        report_jack_into_vec(len);
    }
    report_jack_into_box();
    report_vec_into_jack();
}
