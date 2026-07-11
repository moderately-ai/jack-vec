use std::alloc::{GlobalAlloc, Layout, System};
use std::mem::size_of;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use jack_vec::JackVec;
use jack_vec_comparisons::{
    build_growing, build_nested, fill_vector, BenchVector, NestedWorkload, SmallVec4, SmallVec8,
};
use thin_vec::ThinVec;

struct TrackingAllocator;

static ENABLED: AtomicBool = AtomicBool::new(false);
static LIVE_REQUESTED: AtomicUsize = AtomicUsize::new(0);
static PEAK_REQUESTED: AtomicUsize = AtomicUsize::new(0);
static LIVE_USABLE: AtomicUsize = AtomicUsize::new(0);
static PEAK_USABLE: AtomicUsize = AtomicUsize::new(0);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static REALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static MOVED_REALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static IN_PLACE_REALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static DEALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator;

fn update_peak(peak: &AtomicUsize, candidate: usize) {
    let mut observed = peak.load(Ordering::Relaxed);
    while candidate > observed {
        match peak.compare_exchange_weak(observed, candidate, Ordering::Relaxed, Ordering::Relaxed)
        {
            Ok(_) => break,
            Err(actual) => observed = actual,
        }
    }
}

fn add_live(live: &AtomicUsize, peak: &AtomicUsize, bytes: usize) {
    let current = live.fetch_add(bytes, Ordering::Relaxed) + bytes;
    update_peak(peak, current);
}

#[cfg(target_os = "macos")]
unsafe fn usable_size(pointer: *mut u8, requested: usize) -> usize {
    unsafe extern "C" {
        fn malloc_size(pointer: *const libc::c_void) -> libc::size_t;
    }
    if pointer.is_null() {
        requested
    } else {
        // SAFETY: The pointer denotes a live allocation returned by the
        // platform allocator, as required by malloc_size.
        unsafe { malloc_size(pointer.cast()) }
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
unsafe fn usable_size(pointer: *mut u8, requested: usize) -> usize {
    if pointer.is_null() {
        requested
    } else {
        // SAFETY: The pointer denotes a live glibc allocation, as required by
        // malloc_usable_size.
        unsafe { libc::malloc_usable_size(pointer.cast()) }
    }
}

#[cfg(not(any(target_os = "macos", all(target_os = "linux", target_env = "gnu"))))]
unsafe fn usable_size(_pointer: *mut u8, requested: usize) -> usize {
    requested
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: The caller supplies GlobalAlloc's valid layout and the
        // pointer is returned unchanged from the system allocator.
        let pointer = unsafe { System.alloc(layout) };
        if ENABLED.load(Ordering::Relaxed) && !pointer.is_null() {
            let usable = unsafe { usable_size(pointer, layout.size()) };
            add_live(&LIVE_REQUESTED, &PEAK_REQUESTED, layout.size());
            add_live(&LIVE_USABLE, &PEAK_USABLE, usable);
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if ENABLED.load(Ordering::Relaxed) {
            let usable = unsafe { usable_size(pointer, layout.size()) };
            LIVE_REQUESTED.fetch_sub(layout.size(), Ordering::Relaxed);
            LIVE_USABLE.fetch_sub(usable, Ordering::Relaxed);
            DEALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: The pointer and its original layout are forwarded unchanged
        // to the allocator that created it.
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let tracking = ENABLED.load(Ordering::Relaxed);
        let old_usable = if tracking {
            unsafe { usable_size(pointer, layout.size()) }
        } else {
            0
        };
        // SAFETY: The pointer, original layout, and requested new size are
        // forwarded directly under GlobalAlloc::realloc's contract.
        let replacement = unsafe { System.realloc(pointer, layout, new_size) };
        if tracking && !replacement.is_null() {
            let new_usable = unsafe { usable_size(replacement, new_size) };
            LIVE_REQUESTED.fetch_sub(layout.size(), Ordering::Relaxed);
            add_live(&LIVE_REQUESTED, &PEAK_REQUESTED, new_size);
            LIVE_USABLE.fetch_sub(old_usable, Ordering::Relaxed);
            add_live(&LIVE_USABLE, &PEAK_USABLE, new_usable);
            REALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            if ptr::eq(pointer, replacement) {
                IN_PLACE_REALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            } else {
                MOVED_REALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            }
        }
        replacement
    }
}

#[derive(Clone, Copy)]
struct Snapshot {
    live_requested: usize,
    peak_requested: usize,
    live_usable: usize,
    peak_usable: usize,
    allocations: usize,
    reallocations: usize,
    moved_reallocations: usize,
    in_place_reallocations: usize,
    deallocations: usize,
}

fn snapshot() -> Snapshot {
    Snapshot {
        live_requested: LIVE_REQUESTED.load(Ordering::Relaxed),
        peak_requested: PEAK_REQUESTED.load(Ordering::Relaxed),
        live_usable: LIVE_USABLE.load(Ordering::Relaxed),
        peak_usable: PEAK_USABLE.load(Ordering::Relaxed),
        allocations: ALLOCATIONS.load(Ordering::Relaxed),
        reallocations: REALLOCATIONS.load(Ordering::Relaxed),
        moved_reallocations: MOVED_REALLOCATIONS.load(Ordering::Relaxed),
        in_place_reallocations: IN_PLACE_REALLOCATIONS.load(Ordering::Relaxed),
        deallocations: DEALLOCATIONS.load(Ordering::Relaxed),
    }
}

fn reset() {
    assert!(!ENABLED.load(Ordering::Relaxed));
    for counter in [
        &LIVE_REQUESTED,
        &PEAK_REQUESTED,
        &LIVE_USABLE,
        &PEAK_USABLE,
        &ALLOCATIONS,
        &REALLOCATIONS,
        &MOVED_REALLOCATIONS,
        &IN_PLACE_REALLOCATIONS,
        &DEALLOCATIONS,
    ] {
        counter.store(0, Ordering::Relaxed);
    }
}

struct Row {
    benchmark: &'static str,
    input: &'static str,
    implementation: &'static str,
    element_size: usize,
    container_count: usize,
    owner_bytes: usize,
    inline_capacity: usize,
    len: usize,
    capacity: usize,
    spilled_count: Option<usize>,
    live: Snapshot,
    after_drop: Snapshot,
}

fn measure<T, V, F>(
    benchmark: &'static str,
    input: &'static str,
    container_count: usize,
    build: F,
) -> Row
where
    V: BenchVector<T>,
    F: FnOnce() -> V,
{
    reset();
    ENABLED.store(true, Ordering::Relaxed);
    let value = build();
    let live = snapshot();
    let len = value.len();
    let capacity = value.capacity();
    let spilled_count = value.spilled().map(usize::from);
    drop(value);
    let after_drop = snapshot();
    ENABLED.store(false, Ordering::Relaxed);

    assert_eq!(
        after_drop.live_requested,
        0,
        "requested-byte leak in {benchmark}/{}",
        V::LABEL
    );
    assert_eq!(
        after_drop.live_usable,
        0,
        "usable-byte leak in {benchmark}/{}",
        V::LABEL
    );
    assert!(live.live_usable >= live.live_requested);
    assert_eq!(
        live.reallocations,
        live.moved_reallocations + live.in_place_reallocations
    );

    Row {
        benchmark,
        input,
        implementation: V::LABEL,
        element_size: size_of::<T>(),
        container_count,
        owner_bytes: size_of::<V>() * container_count,
        inline_capacity: V::INLINE_CAPACITY,
        len,
        capacity,
        spilled_count,
        live,
        after_drop,
    }
}

fn nested_row<V: BenchVector<u64>>(workload: NestedWorkload) -> Row {
    reset();
    ENABLED.store(true, Ordering::Relaxed);
    let value = build_nested::<V>(workload, 10_000);
    let live = snapshot();
    let len = value.len();
    let capacity = value.capacity();
    let spilled_count = (V::INLINE_CAPACITY > 0).then(|| {
        value
            .iter()
            .filter(|inner| inner.spilled() == Some(true))
            .count()
    });
    drop(value);
    let after_drop = snapshot();
    ENABLED.store(false, Ordering::Relaxed);

    assert_eq!(
        after_drop.live_requested,
        0,
        "requested-byte leak in nested/{}",
        V::LABEL
    );
    assert_eq!(
        after_drop.live_usable,
        0,
        "usable-byte leak in nested/{}",
        V::LABEL
    );
    assert!(live.live_usable >= live.live_requested);
    assert_eq!(
        live.reallocations,
        live.moved_reallocations + live.in_place_reallocations
    );

    Row {
        benchmark: "nested",
        input: workload.label(),
        implementation: V::LABEL,
        element_size: size_of::<u64>(),
        container_count: 10_000,
        owner_bytes: size_of::<V>() * 10_000,
        inline_capacity: V::INLINE_CAPACITY,
        len,
        capacity,
        spilled_count,
        live,
        after_drop,
    }
}

fn growing_row<V: BenchVector<u64>>(len: usize, label: &'static str) -> Row {
    measure::<u64, V, _>("build_growing", label, 1, || build_growing::<V>(len))
}

fn reserved_row<T, V, F>(len: usize, label: &'static str, make: F) -> Row
where
    V: BenchVector<T>,
    F: Fn(usize) -> T,
{
    measure::<T, V, _>("push_reserved", label, 1, || {
        let mut values = V::with_capacity(len);
        fill_vector(&mut values, len, make);
        values
    })
}

macro_rules! rows_for_all {
    ($rows:expr, $function:ident $(, $argument:expr)*) => {{
        $rows.push($function::<Vec<_>>($($argument),*));
        $rows.push($function::<JackVec<_>>($($argument),*));
        $rows.push($function::<ThinVec<_>>($($argument),*));
        $rows.push($function::<SmallVec4<_>>($($argument),*));
        $rows.push($function::<SmallVec8<_>>($($argument),*));
    }};
}

fn main() {
    let mut rows = Vec::new();
    for workload in NestedWorkload::ALL {
        rows_for_all!(rows, nested_row, workload);
    }
    for &(len, label) in &[(1, "1"), (4, "4"), (1_024, "1024")] {
        rows_for_all!(rows, growing_row, len, label);
        rows.push(reserved_row::<u64, Vec<u64>, _>(len, label, |index| {
            index as u64
        }));
        rows.push(reserved_row::<u64, JackVec<u64>, _>(len, label, |index| {
            index as u64
        }));
        rows.push(reserved_row::<u64, ThinVec<u64>, _>(len, label, |index| {
            index as u64
        }));
        rows.push(reserved_row::<u64, SmallVec4<u64>, _>(
            len,
            label,
            |index| index as u64,
        ));
        rows.push(reserved_row::<u64, SmallVec8<u64>, _>(
            len,
            label,
            |index| index as u64,
        ));
        rows.push(reserved_row::<u8, Vec<u8>, _>(len, label, |index| {
            index as u8
        }));
        rows.push(reserved_row::<u8, JackVec<u8>, _>(len, label, |index| {
            index as u8
        }));
        rows.push(reserved_row::<u8, ThinVec<u8>, _>(len, label, |index| {
            index as u8
        }));
        rows.push(reserved_row::<u8, SmallVec4<u8>, _>(len, label, |index| {
            index as u8
        }));
        rows.push(reserved_row::<u8, SmallVec8<u8>, _>(len, label, |index| {
            index as u8
        }));
    }

    println!("benchmark,input,implementation,element_size,container_count,owner_bytes,inline_capacity,len,capacity,spilled_count,live_requested,peak_requested,after_drop_requested,live_usable,peak_usable,after_drop_usable,allocations,reallocations,moved_reallocations,in_place_reallocations,deallocations");
    for row in rows {
        let spilled = row
            .spilled_count
            .map_or_else(|| "na".to_owned(), |value| value.to_string());
        println!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            row.benchmark,
            row.input,
            row.implementation,
            row.element_size,
            row.container_count,
            row.owner_bytes,
            row.inline_capacity,
            row.len,
            row.capacity,
            spilled,
            row.live.live_requested,
            row.live.peak_requested,
            row.after_drop.live_requested,
            row.live.live_usable,
            row.live.peak_usable,
            row.after_drop.live_usable,
            row.live.allocations,
            row.live.reallocations,
            row.live.moved_reallocations,
            row.live.in_place_reallocations,
            row.after_drop.deallocations,
        );
    }
}
