#![deny(missing_docs)]

//! `JackVec` is exactly the same as `Vec`, except that it stores its `len` and `capacity` in the buffer
//! it allocates.
//!
//! This makes the memory footprint of JackVecs lower; notably in cases where space is reserved for
//! a non-existence `JackVec<T>`. So `Vec<JackVec<T>>` and `Option<JackVec<T>>::None` will waste less
//! space. Being pointer-sized also means it can be passed/stored in registers.
//!
//! Of course, any actually constructed `JackVec` will theoretically have a bigger allocation, but
//! the fuzzy nature of allocators means that might not actually be the case.
//!
//! Properties of `Vec` that are preserved:
//! * `JackVec::new()` doesn't allocate (it points to a statically allocated singleton)
//! * reallocation can be done in place
//! * `size_of::<JackVec<T>>()` == `size_of::<Option<JackVec<T>>>()`
//!
//! Properties of `Vec` that aren't preserved:
//! * `JackVec<T>` can't ever be zero-cost roundtripped to a `Box<[T]>`, `String`, or `*mut T`
//! * `from_raw_parts` doesn't exist
//! * `JackVec` currently doesn't bother to not-allocate for Zero Sized Types (e.g. `JackVec<()>`),
//!   but it could be done if someone cared enough to implement it.
//!
//!
//! # Optional Features
//!
//! ## `const_new`
//!
//! **This feature requires Rust 1.83.**
//!
//! This feature makes `JackVec::new()` a `const fn`.
//!

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "unstable", feature(trusted_len))]
#![allow(clippy::comparison_chain, clippy::missing_safety_doc)]

extern crate alloc;

use alloc::alloc::*;
use alloc::{boxed::Box, vec::Vec};
use core::borrow::*;
use core::cmp::*;
use core::convert::TryFrom;
use core::convert::TryInto;
use core::hash::*;
use core::iter::FromIterator;
use core::marker::PhantomData;
use core::ops::Bound;
use core::ops::{Deref, DerefMut, RangeBounds};
use core::ptr::NonNull;
use core::slice::Iter;
use core::{fmt, mem, ops, ptr, slice};

#[cfg(feature = "malloc_size_of")]
use malloc_size_of::{MallocShallowSizeOf, MallocSizeOf, MallocSizeOfOps};

const MAX_CAP: usize = u32::MAX as usize;

#[cold]
fn capacity_overflow() -> ! {
    panic!("capacity overflow")
}

trait UnwrapCapOverflow<T> {
    fn unwrap_cap_overflow(self) -> T;
}

impl<T> UnwrapCapOverflow<T> for Option<T> {
    fn unwrap_cap_overflow(self) -> T {
        match self {
            Some(val) => val,
            None => capacity_overflow(),
        }
    }
}

impl<T, E> UnwrapCapOverflow<T> for Result<T, E> {
    fn unwrap_cap_overflow(self) -> T {
        match self {
            Ok(val) => val,
            Err(_) => capacity_overflow(),
        }
    }
}

#[inline]
fn header_size(value: usize) -> u32 {
    value.try_into().unwrap_cap_overflow()
}

// The allocation header of a JackVec.
#[repr(C, align(8))]
struct Header {
    _len: u32,
    _cap: u32,
}

impl Header {
    #[inline]
    fn len(&self) -> usize {
        self._len as usize
    }

    #[inline]
    fn set_len(&mut self, len: usize) {
        // Every caller must keep length within the allocation's capacity, and
        // every stored capacity has already passed `header_size`. Avoid a
        // redundant checked conversion on each hot-path length publication.
        debug_assert!(len <= self.cap());
        self._len = len as u32;
    }

    fn cap(&self) -> usize {
        self._cap as usize
    }

    fn set_cap(&mut self, cap: usize) {
        self._cap = header_size(cap);
    }
}

/// Singleton that all empty collections share.
/// Note: can't store non-zero ZSTs, we allocate in that case. We could
/// optimize everything to not do that (basically, make ptr == len and branch
/// on size == 0 in every method), but it's a bunch of work for something that
/// doesn't matter much.
static EMPTY_HEADER: Header = Header { _len: 0, _cap: 0 };

// Utils for computing layouts of allocations

/// Gets the size necessary to allocate a `JackVec<T>` with the give capacity.
///
/// # Panics
///
/// This will panic if isize::MAX is overflowed at any point.
fn alloc_size<T>(cap: usize) -> usize {
    // Compute "real" header size with pointer math
    //
    // We turn everything into isizes here so that we can catch isize::MAX overflow,
    // we never want to allow allocations larger than that!
    let header_size = mem::size_of::<Header>() as isize;
    let padding = padding::<T>() as isize;

    let data_size = if mem::size_of::<T>() == 0 {
        // If we're allocating an array for ZSTs we need a header/padding but no actual
        // space for items, so we don't care about the capacity that was requested!
        0
    } else {
        let cap: isize = cap.try_into().unwrap_cap_overflow();
        let elem_size = mem::size_of::<T>() as isize;
        elem_size.checked_mul(cap).unwrap_cap_overflow()
    };

    let final_size = data_size
        .checked_add(header_size + padding)
        .unwrap_cap_overflow();

    // Ok now we can turn it back into a usize (don't need to worry about negatives)
    final_size as usize
}

/// Gets the padding necessary for the array of a `JackVec<T>`
fn padding<T>() -> usize {
    let alloc_align = alloc_align::<T>();
    let header_size = mem::size_of::<Header>();

    alloc_align.saturating_sub(header_size)
}

/// Gets the align necessary to allocate a `JackVec<T>`
fn alloc_align<T>() -> usize {
    max(mem::align_of::<T>(), mem::align_of::<Header>())
}

/// Gets the layout necessary to allocate a `JackVec<T>`
///
/// # Panics
///
/// Panics if the required size overflows `isize::MAX` when rounded up to the required alignment.
fn layout<T>(cap: usize) -> Layout {
    Layout::from_size_align(alloc_size::<T>(cap), alloc_align::<T>())
        .ok()
        .unwrap_cap_overflow()
}

/// Allocates a header (and array) for a `JackVec<T>` with the given capacity.
///
/// # Panics
///
/// Panics if the required size overflows `isize::MAX` when rounded up to the required alignment.
fn header_with_capacity<T>(cap: usize) -> NonNull<Header> {
    debug_assert!(cap > 0);
    let stored_cap = header_size(cap);
    unsafe {
        let layout = layout::<T>(cap);
        let header = alloc(layout) as *mut Header;

        if header.is_null() {
            handle_alloc_error(layout)
        }

        ptr::write(
            header,
            Header {
                _len: 0,
                _cap: if mem::size_of::<T>() == 0 {
                    // "Infinite" capacity for zero-sized types:
                    MAX_CAP as u32
                } else {
                    stored_cap
                },
            },
        );

        NonNull::new_unchecked(header)
    }
}

/// See the crate's top level documentation for a description of this type.
#[repr(C)]
pub struct JackVec<T> {
    ptr: NonNull<Header>,
    boo: PhantomData<T>,
}

unsafe impl<T: Sync> Sync for JackVec<T> {}
unsafe impl<T: Send> Send for JackVec<T> {}

/// Creates a `JackVec` containing the arguments.
///
/// ```rust
/// #[macro_use] extern crate jackvec;
///
/// fn main() {
///     let v = jack_vec![1, 2, 3];
///     assert_eq!(v.len(), 3);
///     assert_eq!(v[0], 1);
///     assert_eq!(v[1], 2);
///     assert_eq!(v[2], 3);
///
///     let v = jack_vec![1; 3];
///     assert_eq!(v, [1, 1, 1]);
/// }
/// ```
#[macro_export]
macro_rules! jack_vec {
    (@UNIT $($t:tt)*) => (());

    ($elem:expr; $n:expr) => ({
        let mut vec = $crate::JackVec::new();
        vec.resize($n, $elem);
        vec
    });
    () => {$crate::JackVec::new()};
    ($($x:expr),*) => ({
        let len = [$($crate::jack_vec!(@UNIT $x)),*].len();
        let mut vec = $crate::JackVec::with_capacity(len);
        $(vec.push($x);)*
        vec
    });
    ($($x:expr,)*) => ($crate::jack_vec![$($x),*]);
}

impl<T> JackVec<T> {
    /// Creates a new empty JackVec.
    ///
    /// This will not allocate.
    #[cfg(not(feature = "const_new"))]
    pub fn new() -> JackVec<T> {
        JackVec::with_capacity(0)
    }

    /// Creates a new empty JackVec.
    ///
    /// This will not allocate.
    #[cfg(feature = "const_new")]
    pub const fn new() -> JackVec<T> {
        unsafe {
            JackVec {
                ptr: NonNull::new_unchecked(&EMPTY_HEADER as *const Header as *mut Header),
                boo: PhantomData,
            }
        }
    }

    /// Constructs a new, empty `JackVec<T>` with at least the specified capacity.
    ///
    /// The vector will be able to hold at least `capacity` elements without
    /// reallocating. This method is allowed to allocate for more elements than
    /// `capacity`. If `capacity` is 0, the vector will not allocate.
    ///
    /// It is important to note that although the returned vector has the
    /// minimum *capacity* specified, the vector will have a zero *length*.
    ///
    /// If it is important to know the exact allocated capacity of a `JackVec`,
    /// always use the [`capacity`] method after construction.
    ///
    /// **NOTE**: unlike `Vec`, `JackVec` **MUST** allocate once to keep track of non-zero
    /// lengths. As such, we cannot provide the same guarantees about JackVecs
    /// of ZSTs not allocating. However the allocation never needs to be resized
    /// to add more ZSTs, since the underlying array is still length 0.
    ///
    /// [Capacity and reallocation]: #capacity-and-reallocation
    /// [`capacity`]: Vec::capacity
    ///
    /// # Panics
    ///
    /// Panics if the new capacity exceeds `isize::MAX` bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::JackVec;
    ///
    /// let mut vec = JackVec::with_capacity(10);
    ///
    /// // The vector contains no items, even though it has capacity for more
    /// assert_eq!(vec.len(), 0);
    /// assert!(vec.capacity() >= 10);
    ///
    /// // These are all done without reallocating...
    /// for i in 0..10 {
    ///     vec.push(i);
    /// }
    /// assert_eq!(vec.len(), 10);
    /// assert!(vec.capacity() >= 10);
    ///
    /// // ...but this may make the vector reallocate
    /// vec.push(11);
    /// assert_eq!(vec.len(), 11);
    /// assert!(vec.capacity() >= 11);
    ///
    /// // A vector of a zero-sized type will always over-allocate, since no
    /// // space is needed to store the actual elements.
    /// let vec_units = JackVec::<()>::with_capacity(10);
    ///
    /// // assert_eq!(vec_units.capacity(), u32::MAX as usize);
    /// ```
    pub fn with_capacity(cap: usize) -> JackVec<T> {
        // `padding` contains ~static assertions against types that are
        // incompatible with the current feature flags. We also call it to
        // invoke these assertions when getting a pointer to the `JackVec`
        // contents, but since we also get a pointer to the contents in the
        // `Drop` impl, trippng an assertion along that code path causes a
        // double panic. We duplicate the assertion here so that it is
        // testable,
        let _ = padding::<T>();

        if cap > MAX_CAP {
            capacity_overflow();
        }

        if cap == 0 {
            unsafe {
                JackVec {
                    ptr: NonNull::new_unchecked(&EMPTY_HEADER as *const Header as *mut Header),
                    boo: PhantomData,
                }
            }
        } else {
            JackVec {
                ptr: header_with_capacity::<T>(cap),
                boo: PhantomData,
            }
        }
    }

    // Accessor conveniences

    fn ptr(&self) -> *mut Header {
        self.ptr.as_ptr()
    }
    fn header(&self) -> &Header {
        unsafe { self.ptr.as_ref() }
    }
    fn data_raw(&self) -> *mut T {
        // `padding` contains ~static assertions against types that are
        // incompatible with the current feature flags. Even if we don't
        // care about its result, we should always call it before getting
        // a data pointer to guard against invalid types!
        let padding = padding::<T>();

        // Although we ensure the data array is aligned when we allocate,
        // we can't do that with the empty singleton. So when it might not
        // be properly aligned, we substitute in the NonNull::dangling
        // which *is* aligned.
        //
        // To minimize dynamic branches on `cap` for all accesses
        // to the data, we include this guard which should only involve
        // compile-time constants. Ideally this should result in the branch
        // only be included for types with excessive alignment.
        // If the singleton header is naturally aligned for T and requires no
        // padding, one-past-the-header is already a valid empty data pointer.
        let empty_header_is_aligned =
            mem::align_of::<Header>() >= mem::align_of::<T>() && padding == 0;

        unsafe {
            if !empty_header_is_aligned && self.header().cap() == 0 {
                NonNull::dangling().as_ptr()
            } else {
                // This could technically result in overflow, but padding
                // would have to be absurdly large for this to occur.
                let header_size = mem::size_of::<Header>();
                let ptr = self.ptr.as_ptr() as *mut u8;
                ptr.add(header_size + padding) as *mut T
            }
        }
    }

    // This is unsafe when the header is EMPTY_HEADER.
    unsafe fn header_mut(&mut self) -> &mut Header {
        &mut *self.ptr()
    }

    /// Returns the number of elements in the vector, also referred to
    /// as its 'length'.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::jack_vec;
    ///
    /// let a = jack_vec![1, 2, 3];
    /// assert_eq!(a.len(), 3);
    /// ```
    pub fn len(&self) -> usize {
        self.header().len()
    }

    /// Returns `true` if the vector contains no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::JackVec;
    ///
    /// let mut v = JackVec::new();
    /// assert!(v.is_empty());
    ///
    /// v.push(1);
    /// assert!(!v.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the number of elements the vector can hold without
    /// reallocating.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::JackVec;
    ///
    /// let vec: JackVec<i32> = JackVec::with_capacity(10);
    /// assert_eq!(vec.capacity(), 10);
    /// ```
    pub fn capacity(&self) -> usize {
        self.header().cap()
    }

    /// Returns `true` if the vector has the capacity to hold any element.
    pub fn has_capacity(&self) -> bool {
        !self.is_singleton()
    }

    /// Forces the length of the vector to `new_len`.
    ///
    /// This is a low-level operation that maintains none of the normal
    /// invariants of the type. Normally changing the length of a vector
    /// is done using one of the safe operations instead, such as
    /// [`truncate`], [`resize`], [`extend`], or [`clear`].
    ///
    /// [`truncate`]: JackVec::truncate
    /// [`resize`]: JackVec::resize
    /// [`extend`]: JackVec::extend
    /// [`clear`]: JackVec::clear
    ///
    /// # Safety
    ///
    /// - `new_len` must be less than or equal to [`capacity()`].
    /// - The elements at `old_len..new_len` must be initialized.
    ///
    /// [`capacity()`]: JackVec::capacity
    ///
    /// # Examples
    ///
    /// This method can be useful for situations in which the vector
    /// is serving as a buffer for other code, particularly over FFI:
    ///
    /// ```no_run
    /// use jackvec::JackVec;
    ///
    /// # // This is just a minimal skeleton for the doc example;
    /// # // don't use this as a starting point for a real library.
    /// # pub struct StreamWrapper { strm: *mut std::ffi::c_void }
    /// # const Z_OK: i32 = 0;
    /// # extern "C" {
    /// #     fn deflateGetDictionary(
    /// #         strm: *mut std::ffi::c_void,
    /// #         dictionary: *mut u8,
    /// #         dictLength: *mut usize,
    /// #     ) -> i32;
    /// # }
    /// # impl StreamWrapper {
    /// pub fn get_dictionary(&self) -> Option<JackVec<u8>> {
    ///     // Per the FFI method's docs, "32768 bytes is always enough".
    ///     let mut dict = JackVec::with_capacity(32_768);
    ///     let mut dict_length = 0;
    ///     // SAFETY: When `deflateGetDictionary` returns `Z_OK`, it holds that:
    ///     // 1. `dict_length` elements were initialized.
    ///     // 2. `dict_length` <= the capacity (32_768)
    ///     // which makes `set_len` safe to call.
    ///     unsafe {
    ///         // Make the FFI call...
    ///         let r = deflateGetDictionary(self.strm, dict.as_mut_ptr(), &mut dict_length);
    ///         if r == Z_OK {
    ///             // ...and update the length to what was initialized.
    ///             dict.set_len(dict_length);
    ///             Some(dict)
    ///         } else {
    ///             None
    ///         }
    ///     }
    /// }
    /// # }
    /// ```
    ///
    /// While the following example is sound, there is a memory leak since
    /// the inner vectors were not freed prior to the `set_len` call:
    ///
    /// ```no_run
    /// use jackvec::jack_vec;
    ///
    /// let mut vec = jack_vec![jack_vec![1, 0, 0],
    ///                    jack_vec![0, 1, 0],
    ///                    jack_vec![0, 0, 1]];
    /// // SAFETY:
    /// // 1. `old_len..0` is empty so no elements need to be initialized.
    /// // 2. `0 <= capacity` always holds whatever `capacity` is.
    /// unsafe {
    ///     vec.set_len(0);
    /// }
    /// ```
    ///
    /// Normally, here, one would use [`clear`] instead to correctly drop
    /// the contents and thus not leak memory.
    pub unsafe fn set_len(&mut self, len: usize) {
        if self.is_singleton() {
            // A prerequisite of `Vec::set_len` is that `new_len` must be
            // less than or equal to capacity(). The same applies here.
            debug_assert!(len == 0, "invalid set_len({}) on empty JackVec", len);
        } else {
            self.header_mut().set_len(len)
        }
    }

    // For internal use only, when setting the length and it's known to be the non-singleton.
    unsafe fn set_len_non_singleton(&mut self, len: usize) {
        self.header_mut().set_len(len)
    }

    /// Appends an element to the back of a collection.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity exceeds `isize::MAX` bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::jack_vec;
    ///
    /// let mut vec = jack_vec![1, 2];
    /// vec.push(3);
    /// assert_eq!(vec, [1, 2, 3]);
    /// ```
    #[inline]
    pub fn push(&mut self, val: T) {
        let header = self.header();
        let old_len = header.len();
        if old_len == header.cap() {
            self.grow_for_push();
        }
        unsafe {
            // SAFETY: Either the original capacity had room or grow_for_push()
            // reserved room. Growth does not change the length.
            self.push_unchecked_at(old_len, val);
        }
    }

    /// Outlines allocation and layout handling from the common push path.
    #[cold]
    #[inline(never)]
    fn grow_for_push(&mut self) {
        self.reserve(1);
    }

    /// Pushes at a length already loaded and checked by the caller.
    #[inline]
    unsafe fn push_unchecked_at(&mut self, old_len: usize, val: T) {
        debug_assert!(old_len < self.capacity());
        unsafe {
            ptr::write(self.data_raw().add(old_len), val);

            // SAFETY: capacity > len >= 0, so capacity != 0, so this is not a singleton.
            self.set_len_non_singleton(old_len + 1);
        }
    }

    /// Removes the last element from a vector and returns it, or [`None`] if it
    /// is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::jack_vec;
    ///
    /// let mut vec = jack_vec![1, 2, 3];
    /// assert_eq!(vec.pop(), Some(3));
    /// assert_eq!(vec, [1, 2]);
    /// ```
    pub fn pop(&mut self) -> Option<T> {
        let old_len = self.len();
        if old_len == 0 {
            return None;
        }

        unsafe {
            self.set_len_non_singleton(old_len - 1);
            Some(ptr::read(self.data_raw().add(old_len - 1)))
        }
    }

    /// Inserts an element at position `index` within the vector, shifting all
    /// elements after it to the right.
    ///
    /// # Panics
    ///
    /// Panics if `index > len`.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::jack_vec;
    ///
    /// let mut vec = jack_vec![1, 2, 3];
    /// vec.insert(1, 4);
    /// assert_eq!(vec, [1, 4, 2, 3]);
    /// vec.insert(4, 5);
    /// assert_eq!(vec, [1, 4, 2, 3, 5]);
    /// ```
    pub fn insert(&mut self, idx: usize, elem: T) {
        let old_len = self.len();

        assert!(idx <= old_len, "Index out of bounds");
        if old_len == self.capacity() {
            self.reserve(1);
        }
        unsafe {
            let ptr = self.data_raw();
            ptr::copy(ptr.add(idx), ptr.add(idx + 1), old_len - idx);
            ptr::write(ptr.add(idx), elem);
            self.set_len_non_singleton(old_len + 1);
        }
    }

    /// Removes and returns the element at position `index` within the vector,
    /// shifting all elements after it to the left.
    ///
    /// Note: Because this shifts over the remaining elements, it has a
    /// worst-case performance of *O*(*n*). If you don't need the order of elements
    /// to be preserved, use [`swap_remove`] instead. If you'd like to remove
    /// elements from the beginning of the `JackVec`, consider using `std::collections::VecDeque`.
    ///
    /// [`swap_remove`]: JackVec::swap_remove
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::jack_vec;
    ///
    /// let mut v = jack_vec![1, 2, 3];
    /// assert_eq!(v.remove(1), 2);
    /// assert_eq!(v, [1, 3]);
    /// ```
    pub fn remove(&mut self, idx: usize) -> T {
        let old_len = self.len();

        assert!(idx < old_len, "Index out of bounds");

        unsafe {
            self.set_len_non_singleton(old_len - 1);
            let ptr = self.data_raw();
            let val = ptr::read(self.data_raw().add(idx));
            ptr::copy(ptr.add(idx + 1), ptr.add(idx), old_len - idx - 1);
            val
        }
    }

    /// Removes an element from the vector and returns it.
    ///
    /// The removed element is replaced by the last element of the vector.
    ///
    /// This does not preserve ordering, but is *O*(1).
    /// If you need to preserve the element order, use [`remove`] instead.
    ///
    /// [`remove`]: JackVec::remove
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::jack_vec;
    ///
    /// let mut v = jack_vec!["foo", "bar", "baz", "qux"];
    ///
    /// assert_eq!(v.swap_remove(1), "bar");
    /// assert_eq!(v, ["foo", "qux", "baz"]);
    ///
    /// assert_eq!(v.swap_remove(0), "foo");
    /// assert_eq!(v, ["baz", "qux"]);
    /// ```
    pub fn swap_remove(&mut self, idx: usize) -> T {
        let old_len = self.len();

        assert!(idx < old_len, "Index out of bounds");

        unsafe {
            let ptr = self.data_raw();
            ptr::swap(ptr.add(idx), ptr.add(old_len - 1));
            self.set_len_non_singleton(old_len - 1);
            ptr::read(ptr.add(old_len - 1))
        }
    }

    /// Shortens the vector, keeping the first `len` elements and dropping
    /// the rest.
    ///
    /// If `len` is greater than the vector's current length, this has no
    /// effect.
    ///
    /// The [`drain`] method can emulate `truncate`, but causes the excess
    /// elements to be returned instead of dropped.
    ///
    /// Note that this method has no effect on the allocated capacity
    /// of the vector.
    ///
    /// # Examples
    ///
    /// Truncating a five element vector to two elements:
    ///
    /// ```
    /// use jackvec::jack_vec;
    ///
    /// let mut vec = jack_vec![1, 2, 3, 4, 5];
    /// vec.truncate(2);
    /// assert_eq!(vec, [1, 2]);
    /// ```
    ///
    /// No truncation occurs when `len` is greater than the vector's current
    /// length:
    ///
    /// ```
    /// use jackvec::jack_vec;
    ///
    /// let mut vec = jack_vec![1, 2, 3];
    /// vec.truncate(8);
    /// assert_eq!(vec, [1, 2, 3]);
    /// ```
    ///
    /// Truncating when `len == 0` is equivalent to calling the [`clear`]
    /// method.
    ///
    /// ```
    /// use jackvec::jack_vec;
    ///
    /// let mut vec = jack_vec![1, 2, 3];
    /// vec.truncate(0);
    /// assert_eq!(vec, []);
    /// ```
    ///
    /// [`clear`]: JackVec::clear
    /// [`drain`]: JackVec::drain
    pub fn truncate(&mut self, len: usize) {
        unsafe {
            // drop any extra elements
            while len < self.len() {
                // decrement len before the drop_in_place(), so a panic on Drop
                // doesn't re-drop the just-failed value.
                let new_len = self.len() - 1;
                self.set_len_non_singleton(new_len);
                ptr::drop_in_place(self.data_raw().add(new_len));
            }
        }
    }

    /// Clears the vector, removing all values.
    ///
    /// Note that this method has no effect on the allocated capacity
    /// of the vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::jack_vec;
    ///
    /// let mut v = jack_vec![1, 2, 3];
    /// v.clear();
    /// assert!(v.is_empty());
    /// ```
    pub fn clear(&mut self) {
        unsafe {
            // Decrement len even in the case of a panic.
            struct DropGuard<'a, T>(&'a mut JackVec<T>);
            impl<T> Drop for DropGuard<'_, T> {
                fn drop(&mut self) {
                    unsafe {
                        // Could be the singleton.
                        self.0.set_len(0);
                    }
                }
            }
            let guard = DropGuard(self);
            ptr::drop_in_place(&mut guard.0[..]);
        }
    }

    /// Extracts a slice containing the entire vector.
    ///
    /// Equivalent to `&s[..]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::jack_vec;
    /// use std::io::{self, Write};
    /// let buffer = jack_vec![1, 2, 3, 5, 8];
    /// io::sink().write(buffer.as_slice()).unwrap();
    /// ```
    pub fn as_slice(&self) -> &[T] {
        unsafe { slice::from_raw_parts(self.data_raw(), self.len()) }
    }

    /// Extracts a mutable slice of the entire vector.
    ///
    /// Equivalent to `&mut s[..]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::jack_vec;
    /// use std::io::{self, Read};
    /// let mut buffer = vec![0; 3];
    /// io::repeat(0b101).read_exact(buffer.as_mut_slice()).unwrap();
    /// ```
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { slice::from_raw_parts_mut(self.data_raw(), self.len()) }
    }

    /// Reserve capacity for at least `additional` more elements to be inserted.
    ///
    /// May reserve more space than requested, to avoid frequent reallocations.
    ///
    /// Panics if the new capacity overflows `usize`.
    ///
    /// Re-allocates only if `self.capacity() < self.len() + additional`.
    pub fn reserve(&mut self, additional: usize) {
        let len = self.len();
        let old_cap = self.capacity();
        let min_cap = len.checked_add(additional).unwrap_cap_overflow();
        if min_cap <= old_cap {
            return;
        }
        // Ensure the new capacity is at least double, to guarantee exponential growth.
        let double_cap = if old_cap == 0 {
            // skip to 4 because tiny JackVecs are dumb; but not if that would cause overflow
            if mem::size_of::<T>() > (!0) / 8 {
                1
            } else {
                4
            }
        } else {
            old_cap.saturating_mul(2)
        };
        let new_cap = max(min_cap, double_cap);
        unsafe {
            self.reallocate(new_cap);
        }
    }

    /// Reserves the minimum capacity for `additional` more elements to be inserted.
    ///
    /// Panics if the new capacity overflows `usize`.
    ///
    /// Re-allocates only if `self.capacity() < self.len() + additional`.
    pub fn reserve_exact(&mut self, additional: usize) {
        let new_cap = self.len().checked_add(additional).unwrap_cap_overflow();
        let old_cap = self.capacity();
        if new_cap > old_cap {
            unsafe {
                self.reallocate(new_cap);
            }
        }
    }

    /// Shrinks the capacity of the vector as much as possible.
    ///
    /// It will drop down as close as possible to the length but the allocator
    /// may still inform the vector that there is space for a few more elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::JackVec;
    ///
    /// let mut vec = JackVec::with_capacity(10);
    /// vec.extend([1, 2, 3]);
    /// assert_eq!(vec.capacity(), 10);
    /// vec.shrink_to_fit();
    /// assert!(vec.capacity() >= 3);
    /// ```
    pub fn shrink_to_fit(&mut self) {
        let old_cap = self.capacity();
        let new_cap = self.len();
        if new_cap >= old_cap {
            return;
        }
        if new_cap == 0 {
            *self = JackVec::new();
        } else {
            unsafe {
                self.reallocate(new_cap);
            }
        }
    }

    /// Retains only the elements specified by the predicate.
    ///
    /// In other words, remove all elements `e` such that `f(&e)` returns `false`.
    /// This method operates in place and preserves the order of the retained
    /// elements.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[macro_use] extern crate jackvec;
    /// # fn main() {
    /// let mut vec = jack_vec![1, 2, 3, 4];
    /// vec.retain(|&x| x%2 == 0);
    /// assert_eq!(vec, [2, 4]);
    /// # }
    /// ```
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&T) -> bool,
    {
        self.retain_mut(|x| f(&*x));
    }

    /// Retains only the elements specified by the predicate, passing a mutable reference to it.
    ///
    /// In other words, remove all elements `e` such that `f(&mut e)` returns `false`.
    /// This method operates in place and preserves the order of the retained
    /// elements.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[macro_use] extern crate jackvec;
    /// # fn main() {
    /// let mut vec = jack_vec![1, 2, 3, 4, 5];
    /// vec.retain_mut(|x| {
    ///     *x += 1;
    ///     (*x)%2 == 0
    /// });
    /// assert_eq!(vec, [2, 4, 6]);
    /// # }
    /// ```
    pub fn retain_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut T) -> bool,
    {
        let original_len = self.len();
        if original_len == 0 {
            return;
        }

        unsafe {
            // Prevent double-drop if a predicate or destructor panics after a
            // hole has been created. The guard restores the initialized prefix.
            self.set_len_non_singleton(0);
        }

        struct BackshiftOnDrop<'a, T> {
            vec: &'a mut JackVec<T>,
            processed_len: usize,
            deleted_count: usize,
            original_len: usize,
        }

        impl<T> Drop for BackshiftOnDrop<'_, T> {
            fn drop(&mut self) {
                unsafe {
                    if self.deleted_count > 0 {
                        let data = self.vec.data_raw();
                        ptr::copy(
                            data.add(self.processed_len),
                            data.add(self.processed_len - self.deleted_count),
                            self.original_len - self.processed_len,
                        );
                    }
                    self.vec
                        .set_len_non_singleton(self.original_len - self.deleted_count);
                }
            }
        }

        fn process_loop<F, T, const DELETED: bool>(
            original_len: usize,
            f: &mut F,
            guard: &mut BackshiftOnDrop<'_, T>,
        ) where
            F: FnMut(&mut T) -> bool,
        {
            while guard.processed_len != original_len {
                let current = unsafe { &mut *guard.vec.data_raw().add(guard.processed_len) };
                if !f(current) {
                    // Advance before dropping so the guard will not touch this
                    // element again if its destructor panics.
                    guard.processed_len += 1;
                    guard.deleted_count += 1;
                    unsafe {
                        ptr::drop_in_place(current);
                    }
                    if DELETED {
                        continue;
                    }
                    break;
                }

                if DELETED {
                    unsafe {
                        let hole = guard
                            .vec
                            .data_raw()
                            .add(guard.processed_len - guard.deleted_count);
                        ptr::copy_nonoverlapping(current, hole, 1);
                    }
                }
                guard.processed_len += 1;
            }
        }

        let mut guard = BackshiftOnDrop {
            vec: self,
            processed_len: 0,
            deleted_count: 0,
            original_len,
        };
        process_loop::<F, T, false>(original_len, &mut f, &mut guard);
        process_loop::<F, T, true>(original_len, &mut f, &mut guard);
        drop(guard);
    }

    /// Removes consecutive elements in the vector that resolve to the same key.
    ///
    /// If the vector is sorted, this removes all duplicates.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[macro_use] extern crate jackvec;
    /// # fn main() {
    /// let mut vec = jack_vec![10, 20, 21, 30, 20];
    ///
    /// vec.dedup_by_key(|i| *i / 10);
    ///
    /// assert_eq!(vec, [10, 20, 30, 20]);
    /// # }
    /// ```
    pub fn dedup_by_key<F, K>(&mut self, mut key: F)
    where
        F: FnMut(&mut T) -> K,
        K: PartialEq<K>,
    {
        self.dedup_by(|a, b| key(a) == key(b))
    }

    /// Removes consecutive elements in the vector according to a predicate.
    ///
    /// The `same_bucket` function is passed references to two elements from the vector, and
    /// returns `true` if the elements compare equal, or `false` if they do not. The first
    /// argument is the later element and the second is the earlier retained element. Only the
    /// first of adjacent equal items is kept.
    ///
    /// If the vector is sorted, this removes all duplicates.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[macro_use] extern crate jackvec;
    /// # fn main() {
    /// let mut vec = jack_vec!["foo", "bar", "Bar", "baz", "bar"];
    ///
    /// vec.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    ///
    /// assert_eq!(vec, ["foo", "bar", "baz", "bar"]);
    /// # }
    /// ```
    pub fn dedup_by<F>(&mut self, mut same_bucket: F)
    where
        F: FnMut(&mut T, &mut T) -> bool,
    {
        let len = self.len();
        if len <= 1 {
            return;
        }

        // Avoid writes entirely when every element is unique, and establish a
        // gap before using non-overlapping copies for later survivors.
        let start = self.as_mut_ptr();
        let mut first_duplicate = 1;
        while first_duplicate != len {
            let is_duplicate = unsafe {
                same_bucket(
                    &mut *start.add(first_duplicate),
                    &mut *start.add(first_duplicate - 1),
                )
            };
            if is_duplicate {
                break;
            }
            first_duplicate += 1;
        }
        if first_duplicate == len {
            return;
        }

        struct FillGapOnDrop<'a, T> {
            read: usize,
            write: usize,
            vec: &'a mut JackVec<T>,
        }

        impl<T> Drop for FillGapOnDrop<'_, T> {
            fn drop(&mut self) {
                unsafe {
                    let data = self.vec.data_raw();
                    let len = self.vec.len();
                    let items_left = len - self.read;
                    ptr::copy(data.add(self.read), data.add(self.write), items_left);
                    self.vec
                        .set_len_non_singleton(len - (self.read - self.write));
                }
            }
        }

        // Create the first gap before dropping the duplicate. If destruction
        // panics, the guard treats that element as removed and repairs the tail.
        let mut gap = FillGapOnDrop {
            read: first_duplicate + 1,
            write: first_duplicate,
            vec: self,
        };
        unsafe {
            ptr::drop_in_place(start.add(first_duplicate));

            while gap.read < len {
                let current = start.add(gap.read);
                let previous = start.add(gap.write - 1);
                if same_bucket(&mut *current, &mut *previous) {
                    // Advance first so a panicking destructor is excluded from
                    // the suffix repaired by the guard.
                    gap.read += 1;
                    ptr::drop_in_place(current);
                } else {
                    ptr::copy_nonoverlapping(current, start.add(gap.write), 1);
                    gap.write += 1;
                    gap.read += 1;
                }
            }

            gap.vec.set_len_non_singleton(gap.write);
            mem::forget(gap);
        }
    }

    /// Splits the collection into two at the given index.
    ///
    /// Returns a newly allocated vector containing the elements in the range
    /// `[at, len)`. After the call, the original vector will be left containing
    /// the elements `[0, at)` with its previous capacity unchanged.
    ///
    /// # Panics
    ///
    /// Panics if `at > len`.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::jack_vec;
    ///
    /// let mut vec = jack_vec![1, 2, 3];
    /// let vec2 = vec.split_off(1);
    /// assert_eq!(vec, [1]);
    /// assert_eq!(vec2, [2, 3]);
    /// ```
    pub fn split_off(&mut self, at: usize) -> JackVec<T> {
        let old_len = self.len();
        let new_vec_len = old_len - at;

        assert!(at <= old_len, "Index out of bounds");

        unsafe {
            let mut new_vec = JackVec::with_capacity(new_vec_len);

            ptr::copy_nonoverlapping(self.data_raw().add(at), new_vec.data_raw(), new_vec_len);

            new_vec.set_len(new_vec_len); // could be the singleton
            self.set_len(at); // could be the singleton

            new_vec
        }
    }

    /// Moves all the elements of `other` into `self`, leaving `other` empty.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity exceeds `isize::MAX` bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::jack_vec;
    ///
    /// let mut vec = jack_vec![1, 2, 3];
    /// let mut vec2 = jack_vec![4, 5, 6];
    /// vec.append(&mut vec2);
    /// assert_eq!(vec, [1, 2, 3, 4, 5, 6]);
    /// assert_eq!(vec2, []);
    /// ```
    pub fn append(&mut self, other: &mut JackVec<T>) {
        let other_len = other.len();
        if other_len == 0 {
            return;
        }

        self.reserve(other_len);

        unsafe {
            let self_len = self.len();
            ptr::copy_nonoverlapping(other.data_raw(), self.data_raw().add(self_len), other_len);

            // The elements have moved into self. Exclude them from other's
            // initialized prefix before publishing them in self so they can
            // never be dropped twice.
            other.set_len_non_singleton(0);
            self.set_len_non_singleton(self_len + other_len);
        }
    }

    /// Removes the specified range from the vector in bulk, returning all
    /// removed elements as an iterator. If the iterator is dropped before
    /// being fully consumed, it drops the remaining removed elements.
    ///
    /// The returned iterator keeps a mutable borrow on the vector to optimize
    /// its implementation.
    ///
    /// # Panics
    ///
    /// Panics if the starting point is greater than the end point or if
    /// the end point is greater than the length of the vector.
    ///
    /// # Leaking
    ///
    /// If the returned iterator goes out of scope without being dropped (due to
    /// [`mem::forget`], for example), the vector may have lost and leaked
    /// elements arbitrarily, including elements outside the range.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::{JackVec, jack_vec};
    ///
    /// let mut v = jack_vec![1, 2, 3];
    /// let u: JackVec<_> = v.drain(1..).collect();
    /// assert_eq!(v, &[1]);
    /// assert_eq!(u, &[2, 3]);
    ///
    /// // A full range clears the vector, like `clear()` does
    /// v.drain(..);
    /// assert_eq!(v, &[]);
    /// ```
    pub fn drain<R>(&mut self, range: R) -> Drain<'_, T>
    where
        R: RangeBounds<usize>,
    {
        // See comments in the Drain struct itself for details on this
        let len = self.len();
        let start = match range.start_bound() {
            Bound::Included(&n) => n,
            Bound::Excluded(&n) => n + 1,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&n) => n + 1,
            Bound::Excluded(&n) => n,
            Bound::Unbounded => len,
        };
        assert!(start <= end);
        assert!(end <= len);

        unsafe {
            // Set our length to the start bound
            self.set_len(start); // could be the singleton

            let iter = slice::from_raw_parts(self.data_raw().add(start), end - start).iter();

            Drain {
                iter,
                vec: NonNull::from(self),
                end,
                tail: len - end,
            }
        }
    }

    /// Creates a splicing iterator that replaces the specified range in the vector
    /// with the given `replace_with` iterator and yields the removed items.
    /// `replace_with` does not need to be the same length as `range`.
    ///
    /// `range` is removed even if the iterator is not consumed until the end.
    ///
    /// It is unspecified how many elements are removed from the vector
    /// if the `Splice` value is leaked.
    ///
    /// The input iterator `replace_with` is only consumed when the `Splice` value is dropped.
    ///
    /// This is optimal if:
    ///
    /// * The tail (elements in the vector after `range`) is empty,
    /// * or `replace_with` yields fewer or equal elements than `range`’s length
    /// * or the lower bound of its `size_hint()` is exact.
    ///
    /// Otherwise, a temporary vector is allocated and the tail is moved twice.
    ///
    /// # Panics
    ///
    /// Panics if the starting point is greater than the end point or if
    /// the end point is greater than the length of the vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::{JackVec, jack_vec};
    ///
    /// let mut v = jack_vec![1, 2, 3, 4];
    /// let new = [7, 8, 9];
    /// let u: JackVec<_> = v.splice(1..3, new).collect();
    /// assert_eq!(v, &[1, 7, 8, 9, 4]);
    /// assert_eq!(u, &[2, 3]);
    /// ```
    #[inline]
    pub fn splice<R, I>(&mut self, range: R, replace_with: I) -> Splice<'_, I::IntoIter>
    where
        R: RangeBounds<usize>,
        I: IntoIterator<Item = T>,
    {
        Splice {
            drain: self.drain(range),
            replace_with: replace_with.into_iter(),
        }
    }

    /// Creates an iterator which uses a closure to determine if an element should be removed.
    ///
    /// If the closure returns true, then the element is removed and yielded.
    /// If the closure returns false, the element will remain in the vector and will not be yielded
    /// by the iterator.
    ///
    /// If the returned `ExtractIf` is not exhausted, e.g. because it is dropped without iterating
    /// or the iteration short-circuits, then the remaining elements will be retained.
    /// Use [`JackVec::retain`] with a negated predicate if you do not need the returned iterator.
    ///
    /// Using this method is equivalent to the following code:
    ///
    /// ```
    /// # use jackvec::{JackVec, jack_vec};
    /// # let some_predicate = |x: &mut i32| { *x == 2 || *x == 3 || *x == 6 };
    /// # let mut vec = jack_vec![1, 2, 3, 4, 5, 6];
    /// let mut i = 0;
    /// while i < vec.len() {
    ///     if some_predicate(&mut vec[i]) {
    ///         let val = vec.remove(i);
    ///         // your code here
    ///     } else {
    ///         i += 1;
    ///     }
    /// }
    ///
    /// # assert_eq!(vec, jack_vec![1, 4, 5]);
    /// ```
    ///
    /// But `extract_if` is easier to use. `extract_if` is also more efficient,
    /// because it can backshift the elements of the array in bulk.
    ///
    /// Note that `extract_if` also lets you mutate every element in the filter closure,
    /// regardless of whether you choose to keep or remove it.
    ///
    /// # Examples
    ///
    /// Splitting an array into evens and odds, reusing the original allocation:
    ///
    /// ```
    /// use jackvec::{JackVec, jack_vec};
    ///
    /// let mut numbers = jack_vec![1, 2, 3, 4, 5, 6, 8, 9, 11, 13, 14, 15];
    ///
    /// let evens = numbers.extract_if(.., |x| *x % 2 == 0).collect::<JackVec<_>>();
    /// let odds = numbers;
    ///
    /// assert_eq!(evens, jack_vec![2, 4, 6, 8, 14]);
    /// assert_eq!(odds, jack_vec![1, 3, 5, 9, 11, 13, 15]);
    /// ```
    pub fn extract_if<F, R: RangeBounds<usize>>(
        &mut self,
        range: R,
        filter: F,
    ) -> ExtractIf<'_, T, F>
    where
        F: FnMut(&mut T) -> bool,
    {
        // Copy of https://github.com/rust-lang/rust/blob/ee361e8fca1c30e13e7a31cc82b64c045339d3a8/library/core/src/slice/index.rs#L37
        fn slice_index_fail(start: usize, end: usize, len: usize) -> ! {
            if start > len {
                panic!(
                    "range start index {} out of range for slice of length {}",
                    start, len
                )
            }

            if end > len {
                panic!(
                    "range end index {} out of range for slice of length {}",
                    end, len
                )
            }

            if start > end {
                panic!("slice index starts at {} but ends at {}", start, end)
            }

            // Only reachable if the range was a `RangeInclusive` or a
            // `RangeToInclusive`, with `end == len`.
            panic!(
                "range end index {} out of range for slice of length {}",
                end, len
            )
        }

        // Backport of https://github.com/rust-lang/rust/blob/ee361e8fca1c30e13e7a31cc82b64c045339d3a8/library/core/src/slice/index.rs#L855
        pub fn slice_range<R>(range: R, bounds: ops::RangeTo<usize>) -> ops::Range<usize>
        where
            R: ops::RangeBounds<usize>,
        {
            let len = bounds.end;

            let end = match range.end_bound() {
                ops::Bound::Included(&end) if end >= len => slice_index_fail(0, end, len),
                // Cannot overflow because `end < len` implies `end < usize::MAX`.
                ops::Bound::Included(&end) => end + 1,

                ops::Bound::Excluded(&end) if end > len => slice_index_fail(0, end, len),
                ops::Bound::Excluded(&end) => end,
                ops::Bound::Unbounded => len,
            };

            let start = match range.start_bound() {
                ops::Bound::Excluded(&start) if start >= end => slice_index_fail(start, end, len),
                // Cannot overflow because `start < end` implies `start < usize::MAX`.
                ops::Bound::Excluded(&start) => start + 1,

                ops::Bound::Included(&start) if start > end => slice_index_fail(start, end, len),
                ops::Bound::Included(&start) => start,

                ops::Bound::Unbounded => 0,
            };

            ops::Range { start, end }
        }

        let old_len = self.len();
        let ops::Range { start, end } = slice_range(range, ..old_len);

        // Guard against the vec getting leaked (leak amplification)
        unsafe {
            self.set_len(0);
        }
        ExtractIf {
            vec: self,
            idx: start,
            del: 0,
            end,
            old_len,
            pred: filter,
        }
    }

    /// Resize the buffer and update its capacity, without changing the length.
    /// Unsafe because it can cause length to be greater than capacity.
    unsafe fn reallocate(&mut self, new_cap: usize) {
        debug_assert!(new_cap > 0);
        if new_cap > MAX_CAP {
            capacity_overflow();
        }
        if !self.is_singleton() {
            let old_cap = self.capacity();
            let ptr = realloc(
                self.ptr() as *mut u8,
                layout::<T>(old_cap),
                alloc_size::<T>(new_cap),
            ) as *mut Header;

            if ptr.is_null() {
                handle_alloc_error(layout::<T>(new_cap))
            }
            (*ptr).set_cap(new_cap);
            self.ptr = NonNull::new_unchecked(ptr);
        } else {
            self.ptr = header_with_capacity::<T>(new_cap);
        }
    }

    #[inline]
    #[allow(unused_unsafe)]
    fn is_singleton(&self) -> bool {
        unsafe { self.ptr.as_ptr() as *const Header == &EMPTY_HEADER }
    }

    #[inline]
    #[cfg(test)]
    fn has_allocation(&self) -> bool {
        !self.is_singleton()
    }
}

impl<T: Clone> JackVec<T> {
    /// Resizes the `Vec` in-place so that `len()` is equal to `new_len`.
    ///
    /// If `new_len` is greater than `len()`, the `Vec` is extended by the
    /// difference, with each additional slot filled with `value`.
    /// If `new_len` is less than `len()`, the `Vec` is simply truncated.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[macro_use] extern crate jackvec;
    /// # fn main() {
    /// let mut vec = jack_vec!["hello"];
    /// vec.resize(3, "world");
    /// assert_eq!(vec, ["hello", "world", "world"]);
    ///
    /// let mut vec = jack_vec![1, 2, 3, 4];
    /// vec.resize(2, 0);
    /// assert_eq!(vec, [1, 2]);
    /// # }
    /// ```
    pub fn resize(&mut self, new_len: usize, value: T) {
        let old_len = self.len();

        if new_len > old_len {
            let additional = new_len - old_len;
            self.reserve(additional);

            struct SetLenOnDrop<'a, T> {
                vec: &'a mut JackVec<T>,
                initialized_len: usize,
            }

            impl<T> Drop for SetLenOnDrop<'_, T> {
                fn drop(&mut self) {
                    unsafe {
                        self.vec.set_len_non_singleton(self.initialized_len);
                    }
                }
            }

            let data = self.data_raw();
            let mut guard = SetLenOnDrop {
                vec: self,
                initialized_len: old_len,
            };
            for _ in 1..additional {
                unsafe {
                    ptr::write(data.add(guard.initialized_len), value.clone());
                }
                guard.initialized_len += 1;
            }
            unsafe {
                // Move the last value directly rather than cloning it.
                ptr::write(data.add(guard.initialized_len), value);
            }
            guard.initialized_len += 1;
            drop(guard);
        } else if new_len < old_len {
            self.truncate(new_len);
        }
    }

    /// Clones and appends all elements in a slice to the `JackVec`.
    ///
    /// Iterates over the slice `other`, clones each element, and then appends
    /// it to this `JackVec`. The `other` slice is traversed in-order.
    ///
    /// Note that this function is same as [`extend`] except that it is
    /// specialized to work with slices instead. If and when Rust gets
    /// specialization this function will likely be deprecated (but still
    /// available).
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::jack_vec;
    ///
    /// let mut vec = jack_vec![1];
    /// vec.extend_from_slice(&[2, 3, 4]);
    /// assert_eq!(vec, [1, 2, 3, 4]);
    /// ```
    ///
    /// [`extend`]: JackVec::extend
    pub fn extend_from_slice(&mut self, other: &[T]) {
        self.extend(other.iter().cloned())
    }
}

impl<T: PartialEq> JackVec<T> {
    /// Removes consecutive repeated elements in the vector.
    ///
    /// If the vector is sorted, this removes all duplicates.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[macro_use] extern crate jackvec;
    /// # fn main() {
    /// let mut vec = jack_vec![1, 2, 2, 3, 2];
    ///
    /// vec.dedup();
    ///
    /// assert_eq!(vec, [1, 2, 3, 2]);
    /// # }
    /// ```
    pub fn dedup(&mut self) {
        self.dedup_by(|a, b| a == b)
    }
}

impl<T> Drop for JackVec<T> {
    #[inline]
    fn drop(&mut self) {
        #[cold]
        #[inline(never)]
        fn drop_non_singleton<T>(this: &mut JackVec<T>) {
            unsafe {
                ptr::drop_in_place(&mut this[..]);

                dealloc(this.ptr() as *mut u8, layout::<T>(this.capacity()))
            }
        }

        if !self.is_singleton() {
            drop_non_singleton(self);
        }
    }
}

impl<T> Deref for JackVec<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T> DerefMut for JackVec<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

impl<T> Borrow<[T]> for JackVec<T> {
    fn borrow(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T> BorrowMut<[T]> for JackVec<T> {
    fn borrow_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

impl<T> AsRef<[T]> for JackVec<T> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T> Extend<T> for JackVec<T> {
    #[inline]
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = T>,
    {
        let mut iter = iter.into_iter();
        let hint = iter.size_hint().0;
        if hint > 0 {
            self.reserve(hint);

            struct SetLenOnDrop<'a, T> {
                vec: &'a mut JackVec<T>,
                initialized_len: usize,
            }

            impl<T> Drop for SetLenOnDrop<'_, T> {
                fn drop(&mut self) {
                    unsafe {
                        self.vec.set_len_non_singleton(self.initialized_len);
                    }
                }
            }

            let data = self.data_raw();
            let initialized_len = self.len();
            let mut guard = SetLenOnDrop {
                vec: self,
                initialized_len,
            };
            for x in iter.by_ref().take(hint) {
                unsafe {
                    // `reserve(hint)` and `take(hint)` bound every direct write
                    // to initialized_len..initialized_len + hint.
                    ptr::write(data.add(guard.initialized_len), x);
                }
                guard.initialized_len += 1;
            }
            drop(guard);
        }

        // if the hint underestimated the iterator length,
        // push the remaining items with capacity check each time.
        for x in iter {
            self.push(x);
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for JackVec<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T> Hash for JackVec<T>
where
    T: Hash,
{
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self[..].hash(state);
    }
}

impl<T> PartialOrd for JackVec<T>
where
    T: PartialOrd,
{
    #[inline]
    fn partial_cmp(&self, other: &JackVec<T>) -> Option<Ordering> {
        self[..].partial_cmp(&other[..])
    }
}

impl<T> Ord for JackVec<T>
where
    T: Ord,
{
    #[inline]
    fn cmp(&self, other: &JackVec<T>) -> Ordering {
        self[..].cmp(&other[..])
    }
}

impl<A, B> PartialEq<JackVec<B>> for JackVec<A>
where
    A: PartialEq<B>,
{
    #[inline]
    fn eq(&self, other: &JackVec<B>) -> bool {
        self[..] == other[..]
    }
}

impl<A, B> PartialEq<Vec<B>> for JackVec<A>
where
    A: PartialEq<B>,
{
    #[inline]
    fn eq(&self, other: &Vec<B>) -> bool {
        self[..] == other[..]
    }
}

impl<A, B> PartialEq<[B]> for JackVec<A>
where
    A: PartialEq<B>,
{
    #[inline]
    fn eq(&self, other: &[B]) -> bool {
        self[..] == other[..]
    }
}

impl<'a, A, B> PartialEq<&'a [B]> for JackVec<A>
where
    A: PartialEq<B>,
{
    #[inline]
    fn eq(&self, other: &&'a [B]) -> bool {
        self[..] == other[..]
    }
}

// Serde impls based on
// https://github.com/bluss/arrayvec/blob/67ec907a98c0f40c4b76066fed3c1af59d35cf6a/src/arrayvec.rs#L1222-L1267
#[cfg(feature = "serde")]
impl<T: serde::Serialize> serde::Serialize for JackVec<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_seq(self.as_slice())
    }
}

#[cfg(feature = "serde")]
impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for JackVec<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{SeqAccess, Visitor};
        use serde::Deserialize;

        struct JackVecVisitor<T>(PhantomData<T>);

        impl<'de, T: Deserialize<'de>> Visitor<'de> for JackVecVisitor<T> {
            type Value = JackVec<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "a sequence")
            }

            fn visit_seq<SA>(self, mut seq: SA) -> Result<Self::Value, SA::Error>
            where
                SA: SeqAccess<'de>,
            {
                // Same policy as
                // https://github.com/serde-rs/serde/blob/ce0844b9ecc32377b5e4545d759d385a8c46bc6a/serde/src/private/size_hint.rs#L13
                let initial_capacity = seq.size_hint().unwrap_or_default().min(4096);
                let mut values = JackVec::<T>::with_capacity(initial_capacity);

                while let Some(value) = seq.next_element()? {
                    values.push(value);
                }

                Ok(values)
            }
        }

        deserializer.deserialize_seq(JackVecVisitor::<T>(PhantomData))
    }
}

#[cfg(feature = "malloc_size_of")]
impl<T> MallocShallowSizeOf for JackVec<T> {
    fn shallow_size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        if self.capacity() == 0 {
            // We're not a heap pointer.
            return 0;
        }

        unsafe { ops.malloc_size_of(self.ptr() as _) }
    }
}

#[cfg(feature = "malloc_size_of")]
impl<T: MallocSizeOf> MallocSizeOf for JackVec<T> {
    fn size_of(&self, ops: &mut MallocSizeOfOps) -> usize {
        let mut n = self.shallow_size_of(ops);
        for elem in self.iter() {
            n += elem.size_of(ops);
        }
        n
    }
}

macro_rules! array_impls {
    ($($N:expr)*) => {$(
        impl<A, B> PartialEq<[B; $N]> for JackVec<A> where A: PartialEq<B> {
            #[inline]
            fn eq(&self, other: &[B; $N]) -> bool { self[..] == other[..] }
        }

        impl<'a, A, B> PartialEq<&'a [B; $N]> for JackVec<A> where A: PartialEq<B> {
            #[inline]
            fn eq(&self, other: &&'a [B; $N]) -> bool { self[..] == other[..] }
        }
    )*}
}

array_impls! {
    0  1  2  3  4  5  6  7  8  9
    10 11 12 13 14 15 16 17 18 19
    20 21 22 23 24 25 26 27 28 29
    30 31 32
}

impl<T> Eq for JackVec<T> where T: Eq {}

impl<T> IntoIterator for JackVec<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> IntoIter<T> {
        IntoIter {
            vec: self,
            start: 0,
        }
    }
}

impl<'a, T> IntoIterator for &'a JackVec<T> {
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;

    fn into_iter(self) -> slice::Iter<'a, T> {
        self.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut JackVec<T> {
    type Item = &'a mut T;
    type IntoIter = slice::IterMut<'a, T>;

    fn into_iter(self) -> slice::IterMut<'a, T> {
        self.iter_mut()
    }
}

impl<T> Clone for JackVec<T>
where
    T: Clone,
{
    #[inline]
    fn clone(&self) -> JackVec<T> {
        #[cold]
        #[inline(never)]
        fn clone_non_singleton<T: Clone>(this: &JackVec<T>) -> JackVec<T> {
            let len = this.len();
            if len == 0 {
                return JackVec::new();
            }

            let mut new_vec = JackVec::<T>::with_capacity(len);
            let data = new_vec.data_raw();

            struct SetLenOnDrop<'a, T> {
                vec: &'a mut JackVec<T>,
                initialized_len: usize,
            }

            impl<T> Drop for SetLenOnDrop<'_, T> {
                fn drop(&mut self) {
                    unsafe {
                        self.vec.set_len_non_singleton(self.initialized_len);
                    }
                }
            }

            let mut guard = SetLenOnDrop {
                vec: &mut new_vec,
                initialized_len: 0,
            };
            for x in this.iter() {
                unsafe {
                    ptr::write(data.add(guard.initialized_len), x.clone());
                }
                guard.initialized_len += 1;
            }
            drop(guard);
            new_vec
        }

        if self.is_singleton() {
            JackVec::new()
        } else {
            clone_non_singleton(self)
        }
    }
}

impl<T> Default for JackVec<T> {
    fn default() -> JackVec<T> {
        JackVec::new()
    }
}

impl<T> FromIterator<T> for JackVec<T> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> JackVec<T> {
        let mut vec = JackVec::new();
        vec.extend(iter);
        vec
    }
}

impl<T: Clone> From<&[T]> for JackVec<T> {
    /// Allocate a `JackVec<T>` and fill it by cloning `s`'s items.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::{JackVec, jack_vec};
    ///
    /// assert_eq!(JackVec::from(&[1, 2, 3][..]), jack_vec![1, 2, 3]);
    /// ```
    fn from(s: &[T]) -> JackVec<T> {
        s.iter().cloned().collect()
    }
}

impl<T: Clone> From<&mut [T]> for JackVec<T> {
    /// Allocate a `JackVec<T>` and fill it by cloning `s`'s items.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::{JackVec, jack_vec};
    ///
    /// assert_eq!(JackVec::from(&mut [1, 2, 3][..]), jack_vec![1, 2, 3]);
    /// ```
    fn from(s: &mut [T]) -> JackVec<T> {
        s.iter().cloned().collect()
    }
}

impl<T, const N: usize> From<[T; N]> for JackVec<T> {
    /// Allocate a `JackVec<T>` and move `s`'s items into it.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::{JackVec, jack_vec};
    ///
    /// assert_eq!(JackVec::from([1, 2, 3]), jack_vec![1, 2, 3]);
    /// ```
    fn from(s: [T; N]) -> JackVec<T> {
        if N == 0 {
            return JackVec::new();
        }

        let mut vec = JackVec::with_capacity(N);
        let s = mem::ManuallyDrop::new(s);
        unsafe {
            ptr::copy_nonoverlapping(s.as_ptr(), vec.data_raw(), N);
            vec.set_len_non_singleton(N);
        }
        vec
    }
}

impl<T> From<Box<[T]>> for JackVec<T> {
    /// Convert a boxed slice into a vector by transferring ownership of
    /// the existing heap allocation.
    ///
    /// **NOTE:** unlike `std`, this must reallocate to change the layout!
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::{JackVec, jack_vec};
    ///
    /// let b: Box<[i32]> = jack_vec![1, 2, 3].into_iter().collect();
    /// assert_eq!(JackVec::from(b), jack_vec![1, 2, 3]);
    /// ```
    fn from(s: Box<[T]>) -> Self {
        // Can just lean on the fact that `Box<[T]>` -> `Vec<T>` is Free.
        Vec::from(s).into_iter().collect()
    }
}

impl<T> From<Vec<T>> for JackVec<T> {
    /// Convert a `std::Vec` into a `JackVec`.
    ///
    /// **NOTE:** this must reallocate to change the layout!
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::{JackVec, jack_vec};
    ///
    /// let b: Vec<i32> = vec![1, 2, 3];
    /// assert_eq!(JackVec::from(b), jack_vec![1, 2, 3]);
    /// ```
    fn from(mut s: Vec<T>) -> Self {
        let len = s.len();
        if len == 0 {
            return JackVec::new();
        }

        let mut vec = JackVec::with_capacity(len);
        unsafe {
            ptr::copy_nonoverlapping(s.as_ptr(), vec.data_raw(), len);

            // Transfer ownership to JackVec before either container can drop.
            s.set_len(0);
            vec.set_len_non_singleton(len);
        }
        vec
    }
}

impl<T> From<JackVec<T>> for Vec<T> {
    /// Convert a `JackVec` into a `std::Vec`.
    ///
    /// **NOTE:** this must reallocate to change the layout!
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::{JackVec, jack_vec};
    ///
    /// let b: JackVec<i32> = jack_vec![1, 2, 3];
    /// assert_eq!(Vec::from(b), vec![1, 2, 3]);
    /// ```
    fn from(mut s: JackVec<T>) -> Self {
        let len = s.len();
        if len == 0 {
            return Vec::new();
        }

        let mut vec = Vec::with_capacity(len);
        unsafe {
            // JackVec and Vec necessarily use different allocation layouts, but
            // their initialized element regions are both contiguous.
            ptr::copy_nonoverlapping(s.data_raw(), vec.as_mut_ptr(), len);

            // Transfer ownership of the relocated elements before either owner
            // can run Drop. No operation below this point can panic.
            s.set_len_non_singleton(0);
            vec.set_len(len);
        }
        vec
    }
}

impl<T> From<JackVec<T>> for Box<[T]> {
    /// Convert a vector into a boxed slice.
    ///
    /// If `v` has excess capacity, its items will be moved into a
    /// newly-allocated buffer with exactly the right capacity.
    ///
    /// **NOTE:** unlike `std`, this must reallocate to change the layout!
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::{JackVec, jack_vec};
    /// assert_eq!(Box::from(jack_vec![1, 2, 3]), jack_vec![1, 2, 3].into_iter().collect());
    /// ```
    fn from(mut v: JackVec<T>) -> Self {
        let len = v.len();
        if len == 0 {
            return Box::default();
        }

        let mut boxed = Box::<[T]>::new_uninit_slice(len);
        unsafe {
            // The destination is exact-length uninitialized storage for T.
            ptr::copy_nonoverlapping(v.data_raw(), boxed.as_mut_ptr().cast::<T>(), len);

            // Ownership moves to the boxed slice. Nothing below can panic.
            v.set_len_non_singleton(0);
            boxed.assume_init()
        }
    }
}

impl From<&str> for JackVec<u8> {
    /// Allocate a `JackVec<u8>` and fill it with a UTF-8 string.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::{JackVec, jack_vec};
    ///
    /// assert_eq!(JackVec::from("123"), jack_vec![b'1', b'2', b'3']);
    /// ```
    fn from(s: &str) -> JackVec<u8> {
        From::from(s.as_bytes())
    }
}

impl<T, const N: usize> TryFrom<JackVec<T>> for [T; N] {
    type Error = JackVec<T>;

    /// Gets the entire contents of the `JackVec<T>` as an array,
    /// if its size exactly matches that of the requested array.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::{JackVec, jack_vec};
    /// use std::convert::TryInto;
    ///
    /// assert_eq!(jack_vec![1, 2, 3].try_into(), Ok([1, 2, 3]));
    /// assert_eq!(<JackVec<i32>>::new().try_into(), Ok([]));
    /// ```
    ///
    /// If the length doesn't match, the input comes back in `Err`:
    /// ```
    /// use jackvec::{JackVec, jack_vec};
    /// use std::convert::TryInto;
    ///
    /// let r: Result<[i32; 4], _> = (0..10).collect::<JackVec<_>>().try_into();
    /// assert_eq!(r, Err(jack_vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]));
    /// ```
    ///
    /// If you're fine with just getting a prefix of the `JackVec<T>`,
    /// you can call [`.truncate(N)`](JackVec::truncate) first.
    /// ```
    /// use jackvec::{JackVec, jack_vec};
    /// use std::convert::TryInto;
    ///
    /// let mut v = JackVec::from("hello world");
    /// v.sort();
    /// v.truncate(2);
    /// let [a, b]: [_; 2] = v.try_into().unwrap();
    /// assert_eq!(a, b' ');
    /// assert_eq!(b, b'd');
    /// ```
    fn try_from(mut vec: JackVec<T>) -> Result<[T; N], JackVec<T>> {
        if vec.len() != N {
            return Err(vec);
        }

        // SAFETY: `.set_len(0)` is always sound.
        unsafe { vec.set_len(0) };

        // SAFETY: A `JackVec`'s pointer is always aligned properly, and
        // the alignment the array needs is the same as the items.
        // We checked earlier that we have sufficient items.
        // The items will not double-drop as the `set_len`
        // tells the `JackVec` not to also drop them.
        let array = unsafe { ptr::read(vec.data_raw() as *const [T; N]) };
        Ok(array)
    }
}

/// An iterator that moves out of a vector.
///
/// This `struct` is created by the [`JackVec::into_iter`][]
/// (provided by the [`IntoIterator`] trait).
///
/// # Example
///
/// ```
/// use jackvec::jack_vec;
///
/// let v = jack_vec![0, 1, 2];
/// let iter: jackvec::IntoIter<_> = v.into_iter();
/// ```
pub struct IntoIter<T> {
    vec: JackVec<T>,
    start: usize,
}

impl<T> IntoIter<T> {
    /// Returns the remaining items of this iterator as a slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::jack_vec;
    ///
    /// let vec = jack_vec!['a', 'b', 'c'];
    /// let mut into_iter = vec.into_iter();
    /// assert_eq!(into_iter.as_slice(), &['a', 'b', 'c']);
    /// let _ = into_iter.next().unwrap();
    /// assert_eq!(into_iter.as_slice(), &['b', 'c']);
    /// ```
    pub fn as_slice(&self) -> &[T] {
        unsafe { slice::from_raw_parts(self.vec.data_raw().add(self.start), self.len()) }
    }

    /// Returns the remaining items of this iterator as a mutable slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::jack_vec;
    ///
    /// let vec = jack_vec!['a', 'b', 'c'];
    /// let mut into_iter = vec.into_iter();
    /// assert_eq!(into_iter.as_slice(), &['a', 'b', 'c']);
    /// into_iter.as_mut_slice()[2] = 'z';
    /// assert_eq!(into_iter.next().unwrap(), 'a');
    /// assert_eq!(into_iter.next().unwrap(), 'b');
    /// assert_eq!(into_iter.next().unwrap(), 'z');
    /// ```
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { &mut *self.as_raw_mut_slice() }
    }

    fn as_raw_mut_slice(&mut self) -> *mut [T] {
        unsafe { ptr::slice_from_raw_parts_mut(self.vec.data_raw().add(self.start), self.len()) }
    }
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        if self.start == self.vec.len() {
            None
        } else {
            unsafe {
                let old_start = self.start;
                self.start += 1;
                Some(ptr::read(self.vec.data_raw().add(old_start)))
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.vec.len() - self.start;
        (len, Some(len))
    }
}

impl<T> DoubleEndedIterator for IntoIter<T> {
    fn next_back(&mut self) -> Option<T> {
        if self.start == self.vec.len() {
            None
        } else {
            self.vec.pop()
        }
    }
}

impl<T> ExactSizeIterator for IntoIter<T> {}

impl<T> core::iter::FusedIterator for IntoIter<T> {}

// SAFETY: the length calculation is trivial, we're an array! And if it's wrong we're So Screwed.
#[cfg(feature = "unstable")]
unsafe impl<T> core::iter::TrustedLen for IntoIter<T> {}

impl<T> Drop for IntoIter<T> {
    #[inline]
    fn drop(&mut self) {
        #[cold]
        #[inline(never)]
        fn drop_non_singleton<T>(this: &mut IntoIter<T>) {
            // Leak on panic.
            struct DropGuard<'a, T>(&'a mut IntoIter<T>);
            impl<T> Drop for DropGuard<'_, T> {
                fn drop(&mut self) {
                    unsafe {
                        self.0.vec.set_len_non_singleton(0);
                    }
                }
            }
            unsafe {
                let guard = DropGuard(this);
                ptr::drop_in_place(&mut guard.0.vec[guard.0.start..]);
            }
        }

        if !self.vec.is_singleton() {
            drop_non_singleton(self);
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for IntoIter<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("IntoIter").field(&self.as_slice()).finish()
    }
}

impl<T> AsRef<[T]> for IntoIter<T> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T: Clone> Clone for IntoIter<T> {
    #[allow(clippy::into_iter_on_ref)]
    fn clone(&self) -> Self {
        // Just create a new `JackVec` from the remaining elements and IntoIter it
        self.as_slice()
            .into_iter()
            .cloned()
            .collect::<JackVec<_>>()
            .into_iter()
    }
}

/// A draining iterator for `JackVec<T>`.
///
/// This `struct` is created by [`JackVec::drain`].
/// See its documentation for more.
///
/// # Example
///
/// ```
/// use jackvec::jack_vec;
///
/// let mut v = jack_vec![0, 1, 2];
/// let iter: jackvec::Drain<_> = v.drain(..);
/// ```
pub struct Drain<'a, T> {
    // Ok so JackVec::drain takes a range of the JackVec and yields the contents by-value,
    // then backshifts the array. During iteration the array is in an unsound state
    // (big deinitialized hole in it), and this is very dangerous.
    //
    // Our first line of defense is the borrow checker: we have a mutable borrow, so nothing
    // can access the JackVec while we exist. As long as we make sure the JackVec is in a valid
    // state again before we release the borrow, everything should be A-OK! We do this cleanup
    // in our Drop impl.
    //
    // Unfortunately, that's unsound, because mem::forget exists and The Leakpocalypse Is Real.
    // So we can't actually guarantee our destructor runs before our borrow expires. Thankfully
    // this isn't fatal: we can just set the JackVec's len to 0 at the start, so if anyone
    // leaks the Drain, we just leak everything the JackVec contained out of spite! If they
    // *don't* leak us then we can properly repair the len in our Drop impl. This is known
    // as "leak amplification", and is the same approach std uses.
    //
    // But we can do slightly better than setting the len to 0! The drain breaks us up into
    // these parts:
    //
    // ```text
    //
    // [A, B, C, D, E, F, G, H, _, _]
    //  ____  __________  ____  ____
    //   |         |        |     |
    // prefix    drain     tail  spare-cap
    // ```
    //
    // As the drain iterator is consumed from both ends (DoubleEnded!), we'll start to look
    // like this:
    //
    // ```text
    // [A, B, _, _, E, _, G, H, _, _]
    //  ____  __________  ____  ____
    //   |         |        |     |
    // prefix    drain     tail   spare-cap
    // ```
    //
    // Note that the prefix is always valid and untouched, as such we can set the len
    // to the prefix when doing leak-amplification. As a bonus, we can use this value
    // to remember where the drain range starts. At the end we'll look like this
    // (we exhaust ourselves in our Drop impl):
    //
    // ```text
    // [A, B, _, _, _, _, G, H, _, _]
    // _____  __________  _____ ____
    //   |         |        |     |
    //  len      drain     tail  spare-cap
    // ```
    //
    // And need to become this:
    //
    // ```text
    // [A, B, G, H, _, _, _, _, _, _]
    // ___________  ________________
    //     |               |
    //    len          spare-cap
    // ```
    //
    // All this requires is moving the tail back to the prefix (stored in `len`)
    // and setting `len` to `len + tail_len` to undo the leak amplification.
    /// An iterator over the elements we're removing.
    ///
    /// As we go we'll be `read`ing out of the shared refs yielded by this.
    /// It's ok to use Iter here because it promises to only take refs to the parts
    /// we haven't yielded yet.
    iter: Iter<'a, T>,
    /// The actual JackVec, which we need to hold onto to undo the leak amplification
    /// and backshift the tail into place. This should only be accessed when we're
    /// completely done with the Iter in the `drop` impl of this type (or miri will get mad).
    ///
    /// Since we set the `len` of this to be before `Iter`, we can use that `len`
    /// to retrieve the index of the start of the drain range later.
    vec: NonNull<JackVec<T>>,
    /// The one-past-the-end index of the drain range, or equivalently the start of the tail.
    end: usize,
    /// The length of the tail.
    tail: usize,
}

impl<'a, T> Iterator for Drain<'a, T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.iter.next().map(|x| unsafe { ptr::read(x) })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a, T> DoubleEndedIterator for Drain<'a, T> {
    fn next_back(&mut self) -> Option<T> {
        self.iter.next_back().map(|x| unsafe { ptr::read(x) })
    }
}

impl<'a, T> ExactSizeIterator for Drain<'a, T> {}

// SAFETY: we need to keep track of this perfectly Or Else anyway!
#[cfg(feature = "unstable")]
unsafe impl<T> core::iter::TrustedLen for Drain<'_, T> {}

impl<T> core::iter::FusedIterator for Drain<'_, T> {}

impl<'a, T> Drop for Drain<'a, T> {
    fn drop(&mut self) {
        // Consume the rest of the iterator.
        for _ in self.by_ref() {}

        // Move the tail over the drained items, and update the length.
        unsafe {
            let vec = self.vec.as_mut();

            // Don't mutate the empty singleton!
            if !vec.is_singleton() {
                let old_len = vec.len();
                let start = vec.data_raw().add(old_len);
                let end = vec.data_raw().add(self.end);
                ptr::copy(end, start, self.tail);
                vec.set_len_non_singleton(old_len + self.tail);
            }
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for Drain<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Drain").field(&self.iter.as_slice()).finish()
    }
}

impl<'a, T> Drain<'a, T> {
    /// Returns the remaining items of this iterator as a slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use jackvec::jack_vec;
    ///
    /// let mut vec = jack_vec!['a', 'b', 'c'];
    /// let mut drain = vec.drain(..);
    /// assert_eq!(drain.as_slice(), &['a', 'b', 'c']);
    /// let _ = drain.next().unwrap();
    /// assert_eq!(drain.as_slice(), &['b', 'c']);
    /// ```
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        // SAFETY: this is A-OK because the elements that the underlying
        // iterator still points at are still logically initialized and contiguous.
        self.iter.as_slice()
    }
}

impl<'a, T> AsRef<[T]> for Drain<'a, T> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

/// A splicing iterator for `JackVec`.
///
/// This struct is created by [`JackVec::splice`][].
/// See its documentation for more.
///
/// # Example
///
/// ```
/// use jackvec::jack_vec;
///
/// let mut v = jack_vec![0, 1, 2];
/// let new = [7, 8];
/// let iter: jackvec::Splice<_> = v.splice(1.., new);
/// ```
#[derive(Debug)]
pub struct Splice<'a, I: Iterator + 'a> {
    drain: Drain<'a, I::Item>,
    replace_with: I,
}

impl<I: Iterator> Iterator for Splice<'_, I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.drain.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.drain.size_hint()
    }
}

impl<I: Iterator> DoubleEndedIterator for Splice<'_, I> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.drain.next_back()
    }
}

impl<I: Iterator> ExactSizeIterator for Splice<'_, I> {}

impl<I: Iterator> Drop for Splice<'_, I> {
    fn drop(&mut self) {
        // Ensure we've fully drained out the range
        self.drain.by_ref().for_each(drop);

        unsafe {
            // If there's no tail elements, then the inner JackVec is already
            // correct and we can just extend it like normal.
            if self.drain.tail == 0 {
                self.drain.vec.as_mut().extend(self.replace_with.by_ref());
                return;
            }

            // First fill the range left by drain().
            if !self.drain.fill(&mut self.replace_with) {
                return;
            }

            // There may be more elements. Use the lower bound as an estimate.
            let (lower_bound, _upper_bound) = self.replace_with.size_hint();
            if lower_bound > 0 {
                self.drain.move_tail(lower_bound);
                if !self.drain.fill(&mut self.replace_with) {
                    return;
                }
            }

            // Collect any remaining elements.
            // This is a zero-length vector which does not allocate if `lower_bound` was exact.
            let mut collected = self
                .replace_with
                .by_ref()
                .collect::<Vec<I::Item>>()
                .into_iter();
            // Now we have an exact count.
            if collected.len() > 0 {
                self.drain.move_tail(collected.len());
                let filled = self.drain.fill(&mut collected);
                debug_assert!(filled);
                debug_assert_eq!(collected.len(), 0);
            }
        }
        // Let `Drain::drop` move the tail back if necessary and restore `vec.len`.
    }
}

/// Private helper methods for `Splice::drop`
impl<T> Drain<'_, T> {
    /// The range from `self.vec.len` to `self.tail_start` contains elements
    /// that have been moved out.
    /// Fill that range as much as possible with new elements from the `replace_with` iterator.
    /// Returns `true` if we filled the entire range. (`replace_with.next()` didn’t return `None`.)
    unsafe fn fill<I: Iterator<Item = T>>(&mut self, replace_with: &mut I) -> bool {
        let vec = unsafe { self.vec.as_mut() };
        let range_start = vec.len();
        let range_end = self.end;
        let range_slice = unsafe {
            slice::from_raw_parts_mut(vec.data_raw().add(range_start), range_end - range_start)
        };

        for place in range_slice {
            if let Some(new_item) = replace_with.next() {
                unsafe { ptr::write(place, new_item) };
                vec.set_len(vec.len() + 1);
            } else {
                return false;
            }
        }
        true
    }

    /// Makes room for inserting more elements before the tail.
    unsafe fn move_tail(&mut self, additional: usize) {
        let vec = unsafe { self.vec.as_mut() };
        debug_assert_eq!(vec.len(), self.end);
        let additional_capacity = self.tail.checked_add(additional).unwrap_cap_overflow();
        vec.reserve(additional_capacity);

        let new_tail_start = self.end + additional;
        unsafe {
            let src = vec.data_raw().add(self.end);
            let dst = vec.data_raw().add(new_tail_start);
            ptr::copy(src, dst, self.tail);
        }
        self.end = new_tail_start;
    }
}

/// An iterator for [`JackVec`] which uses a closure to determine if an element should be removed.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct ExtractIf<'a, T, F> {
    vec: &'a mut JackVec<T>,
    /// The index of the item that will be inspected by the next call to `next`.
    idx: usize,
    /// Elements at and beyond this point will be retained. Must be equal or smaller than `old_len`.
    end: usize,
    /// The number of items that have been drained (removed) thus far.
    del: usize,
    /// The original length of `vec` prior to draining.
    old_len: usize,
    /// The filter test predicate.
    pred: F,
}

impl<T, F> Iterator for ExtractIf<'_, T, F>
where
    F: FnMut(&mut T) -> bool,
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        unsafe {
            let v = self.vec.data_raw();
            while self.idx < self.end {
                let i = self.idx;
                let drained = (self.pred)(&mut *v.add(i));
                // Update the index *after* the predicate is called. If the index
                // is updated prior and the predicate panics, the element at this
                // index would be leaked.
                self.idx += 1;
                if drained {
                    self.del += 1;
                    return Some(ptr::read(v.add(i)));
                } else if self.del > 0 {
                    let del = self.del;
                    let src: *const T = v.add(i);
                    let dst: *mut T = v.add(i - del);
                    ptr::copy_nonoverlapping(src, dst, 1);
                }
            }
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.end - self.idx))
    }
}

impl<A, F> Drop for ExtractIf<'_, A, F> {
    fn drop(&mut self) {
        unsafe {
            if self.idx < self.old_len && self.del > 0 {
                // This is a pretty messed up state, and there isn't really an
                // obviously right thing to do. We don't want to keep trying
                // to execute `pred`, so we just backshift all the unprocessed
                // elements and tell the vec that they still exist. The backshift
                // is required to prevent a double-drop of the last successfully
                // drained item prior to a panic in the predicate.
                let ptr = self.vec.data_raw();
                let src = ptr.add(self.idx);
                let dst = src.sub(self.del);
                let tail_len = self.old_len - self.idx;
                src.copy_to(dst, tail_len);
            }

            self.vec.set_len(self.old_len - self.del);
        }
    }
}

/// Write is implemented for `JackVec<u8>` by appending to the vector.
/// The vector will grow as needed.
/// This implementation is identical to the one for `Vec<u8>`.
#[cfg(feature = "std")]
impl std::io::Write for JackVec<u8> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.extend_from_slice(buf);
        Ok(buf.len())
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        self.extend_from_slice(buf);
        Ok(())
    }

    #[inline]
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// TODO: a million Index impls

#[cfg(test)]
mod tests {
    use super::{JackVec, MAX_CAP};
    use crate::alloc::{string::ToString, vec};

    #[test]
    fn test_size_of() {
        use core::mem::size_of;
        assert_eq!(size_of::<JackVec<u8>>(), size_of::<&u8>());

        assert_eq!(size_of::<Option<JackVec<u8>>>(), size_of::<&u8>());
        assert_eq!(size_of::<super::Header>(), 8);
    }

    #[test]
    fn test_max_capacity_zst() {
        let vec = JackVec::<()>::with_capacity(MAX_CAP);
        assert_eq!(vec.capacity(), MAX_CAP);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    #[should_panic(expected = "capacity overflow")]
    fn test_capacity_above_u32_max() {
        let _ = JackVec::<()>::with_capacity(MAX_CAP + 1);
    }

    #[test]
    fn test_drop_empty() {
        JackVec::<u8>::new();
    }

    #[test]
    #[should_panic]
    fn test_cap_plus_header_rounded_up_overflows() {
        let _ = JackVec::<u8>::with_capacity(isize::MAX as usize - size_of::<super::Header>());
    }

    #[test]
    fn test_data_ptr_alignment() {
        let v = JackVec::<u16>::new();
        assert!(v.data_raw() as usize % core::mem::align_of::<u16>() == 0);

        let v = JackVec::<u32>::new();
        assert!(v.data_raw() as usize % core::mem::align_of::<u32>() == 0);

        let v = JackVec::<u64>::new();
        assert!(v.data_raw() as usize % core::mem::align_of::<u64>() == 0);
    }

    #[test]
    fn test_overaligned_type() {
        #[repr(align(16))]
        #[allow(unused)]
        struct Align16(u8);

        let v = JackVec::<Align16>::new();
        assert!(v.data_raw() as usize % 16 == 0);
    }

    #[test]
    fn test_partial_eq() {
        assert_eq!(jack_vec![0], jack_vec![0]);
        assert_ne!(jack_vec![0], jack_vec![1]);
        assert_eq!(jack_vec![1, 2, 3], vec![1, 2, 3]);
    }

    #[test]
    fn test_alloc() {
        let mut v = JackVec::new();
        assert!(!v.has_allocation());
        v.push(1);
        assert!(v.has_allocation());
        v.pop();
        assert!(v.has_allocation());
        v.shrink_to_fit();
        assert!(!v.has_allocation());
        v.reserve(64);
        assert!(v.has_allocation());
        v = JackVec::with_capacity(64);
        assert!(v.has_allocation());
        v = JackVec::with_capacity(0);
        assert!(!v.has_allocation());
    }

    #[test]
    fn test_drain_items() {
        let mut vec = jack_vec![1, 2, 3];
        let mut vec2 = jack_vec![];
        for i in vec.drain(..) {
            vec2.push(i);
        }
        assert_eq!(vec, []);
        assert_eq!(vec2, [1, 2, 3]);
    }

    #[test]
    fn test_drain_items_reverse() {
        let mut vec = jack_vec![1, 2, 3];
        let mut vec2 = jack_vec![];
        for i in vec.drain(..).rev() {
            vec2.push(i);
        }
        assert_eq!(vec, []);
        assert_eq!(vec2, [3, 2, 1]);
    }

    #[test]
    fn test_drain_items_zero_sized() {
        let mut vec = jack_vec![(), (), ()];
        let mut vec2 = jack_vec![];
        for i in vec.drain(..) {
            vec2.push(i);
        }
        assert_eq!(vec, []);
        assert_eq!(vec2, [(), (), ()]);
    }

    #[test]
    #[should_panic]
    fn test_drain_out_of_bounds() {
        let mut v = jack_vec![1, 2, 3, 4, 5];
        v.drain(5..6);
    }

    #[test]
    fn test_drain_range() {
        let mut v = jack_vec![1, 2, 3, 4, 5];
        for _ in v.drain(4..) {}
        assert_eq!(v, &[1, 2, 3, 4]);

        let mut v: JackVec<_> = (1..6).map(|x| x.to_string()).collect();
        for _ in v.drain(1..4) {}
        assert_eq!(v, &[1.to_string(), 5.to_string()]);

        let mut v: JackVec<_> = (1..6).map(|x| x.to_string()).collect();
        for _ in v.drain(1..4).rev() {}
        assert_eq!(v, &[1.to_string(), 5.to_string()]);

        let mut v: JackVec<_> = jack_vec![(); 5];
        for _ in v.drain(1..4).rev() {}
        assert_eq!(v, &[(), ()]);
    }

    #[test]
    fn test_drain_max_vec_size() {
        let mut v = JackVec::<()>::with_capacity(MAX_CAP);
        unsafe {
            v.set_len(MAX_CAP);
        }
        for _ in v.drain(MAX_CAP - 1..) {}
        assert_eq!(v.len(), MAX_CAP - 1);
    }

    #[test]
    fn test_clear() {
        let mut v = JackVec::<i32>::new();
        assert_eq!(v.len(), 0);
        assert_eq!(v.capacity(), 0);
        assert_eq!(&v[..], &[]);

        v.clear();
        assert_eq!(v.len(), 0);
        assert_eq!(v.capacity(), 0);
        assert_eq!(&v[..], &[]);

        v.push(1);
        v.push(2);
        assert_eq!(v.len(), 2);
        assert!(v.capacity() >= 2);
        assert_eq!(&v[..], &[1, 2]);

        v.clear();
        assert_eq!(v.len(), 0);
        assert!(v.capacity() >= 2);
        assert_eq!(&v[..], &[]);

        v.push(3);
        v.push(4);
        assert_eq!(v.len(), 2);
        assert!(v.capacity() >= 2);
        assert_eq!(&v[..], &[3, 4]);

        v.clear();
        assert_eq!(v.len(), 0);
        assert!(v.capacity() >= 2);
        assert_eq!(&v[..], &[]);

        v.clear();
        assert_eq!(v.len(), 0);
        assert!(v.capacity() >= 2);
        assert_eq!(&v[..], &[]);
    }

    #[test]
    fn test_empty_singleton_torture() {
        {
            let mut v = JackVec::<i32>::new();
            assert_eq!(v.len(), 0);
            assert_eq!(v.capacity(), 0);
            assert!(v.is_empty());
            assert_eq!(&v[..], &[]);
            assert_eq!(&mut v[..], &mut []);

            assert_eq!(v.pop(), None);
            assert_eq!(v.len(), 0);
            assert_eq!(v.capacity(), 0);
            assert_eq!(&v[..], &[]);
        }

        {
            let v = JackVec::<i32>::new();
            assert_eq!(v.into_iter().count(), 0);

            let v = JackVec::<i32>::new();
            #[allow(clippy::never_loop)]
            for _ in v.into_iter() {
                unreachable!();
            }
        }

        {
            let mut v = JackVec::<i32>::new();
            assert_eq!(v.drain(..).len(), 0);

            #[allow(clippy::never_loop)]
            for _ in v.drain(..) {
                unreachable!()
            }

            assert_eq!(v.len(), 0);
            assert_eq!(v.capacity(), 0);
            assert_eq!(&v[..], &[]);
        }

        {
            let mut v = JackVec::<i32>::new();
            assert_eq!(v.splice(.., []).len(), 0);

            #[allow(clippy::never_loop)]
            for _ in v.splice(.., []) {
                unreachable!()
            }

            assert_eq!(v.len(), 0);
            assert_eq!(v.capacity(), 0);
            assert_eq!(&v[..], &[]);
        }

        {
            let mut v = JackVec::<i32>::new();
            v.truncate(1);
            assert_eq!(v.len(), 0);
            assert_eq!(v.capacity(), 0);
            assert_eq!(&v[..], &[]);

            v.truncate(0);
            assert_eq!(v.len(), 0);
            assert_eq!(v.capacity(), 0);
            assert_eq!(&v[..], &[]);
        }

        {
            let mut v = JackVec::<i32>::new();
            v.shrink_to_fit();
            assert_eq!(v.len(), 0);
            assert_eq!(v.capacity(), 0);
            assert_eq!(&v[..], &[]);
        }

        {
            let mut v = JackVec::<i32>::new();
            let new = v.split_off(0);
            assert_eq!(v.len(), 0);
            assert_eq!(v.capacity(), 0);
            assert_eq!(&v[..], &[]);

            assert_eq!(new.len(), 0);
            assert_eq!(new.capacity(), 0);
            assert_eq!(&new[..], &[]);
        }

        {
            let mut v = JackVec::<i32>::new();
            let mut other = JackVec::<i32>::new();
            v.append(&mut other);

            assert_eq!(v.len(), 0);
            assert_eq!(v.capacity(), 0);
            assert_eq!(&v[..], &[]);

            assert_eq!(other.len(), 0);
            assert_eq!(other.capacity(), 0);
            assert_eq!(&other[..], &[]);
        }

        {
            let mut v = JackVec::<i32>::new();
            v.reserve(0);

            assert_eq!(v.len(), 0);
            assert_eq!(v.capacity(), 0);
            assert_eq!(&v[..], &[]);
        }

        {
            let mut v = JackVec::<i32>::new();
            v.reserve_exact(0);

            assert_eq!(v.len(), 0);
            assert_eq!(v.capacity(), 0);
            assert_eq!(&v[..], &[]);
        }

        {
            let mut v = JackVec::<i32>::new();
            v.reserve(0);

            assert_eq!(v.len(), 0);
            assert_eq!(v.capacity(), 0);
            assert_eq!(&v[..], &[]);
        }

        {
            let v = JackVec::<i32>::with_capacity(0);

            assert_eq!(v.len(), 0);
            assert_eq!(v.capacity(), 0);
            assert_eq!(&v[..], &[]);
        }

        {
            let v = JackVec::<i32>::default();

            assert_eq!(v.len(), 0);
            assert_eq!(v.capacity(), 0);
            assert_eq!(&v[..], &[]);
        }

        {
            let mut v = JackVec::<i32>::new();
            v.retain(|_| unreachable!());

            assert_eq!(v.len(), 0);
            assert_eq!(v.capacity(), 0);
            assert_eq!(&v[..], &[]);
        }

        {
            let mut v = JackVec::<i32>::new();
            v.retain_mut(|_| unreachable!());

            assert_eq!(v.len(), 0);
            assert_eq!(v.capacity(), 0);
            assert_eq!(&v[..], &[]);
        }

        {
            let mut v = JackVec::<i32>::new();
            v.dedup_by_key(|x| *x);

            assert_eq!(v.len(), 0);
            assert_eq!(v.capacity(), 0);
            assert_eq!(&v[..], &[]);
        }

        {
            let mut v = JackVec::<i32>::new();
            v.dedup_by(|_, _| unreachable!());

            assert_eq!(v.len(), 0);
            assert_eq!(v.capacity(), 0);
            assert_eq!(&v[..], &[]);
        }

        {
            let v = JackVec::<i32>::new();
            let v = v.clone();

            assert_eq!(v.len(), 0);
            assert_eq!(v.capacity(), 0);
            assert_eq!(&v[..], &[]);
        }
    }

    #[test]
    fn test_clone() {
        let mut v = JackVec::<i32>::new();
        assert!(v.is_singleton());
        v.push(0);
        v.pop();
        assert!(!v.is_singleton());

        let v2 = v.clone();
        assert!(v2.is_singleton());
    }
}

#[cfg(test)]
mod std_tests {
    #![allow(clippy::reversed_empty_ranges)]

    use super::*;
    use crate::alloc::{
        format,
        string::{String, ToString},
    };
    use core::mem::size_of;

    struct DropCounter<'a> {
        count: &'a mut u32,
    }

    impl<'a> Drop for DropCounter<'a> {
        fn drop(&mut self) {
            *self.count += 1;
        }
    }

    #[test]
    fn test_small_vec_struct() {
        assert!(size_of::<JackVec<u8>>() == size_of::<usize>());
    }

    #[test]
    fn test_double_drop() {
        struct TwoVec<T> {
            x: JackVec<T>,
            y: JackVec<T>,
        }

        let (mut count_x, mut count_y) = (0, 0);
        {
            let mut tv = TwoVec {
                x: JackVec::new(),
                y: JackVec::new(),
            };
            tv.x.push(DropCounter {
                count: &mut count_x,
            });
            tv.y.push(DropCounter {
                count: &mut count_y,
            });

            // If JackVec had a drop flag, here is where it would be zeroed.
            // Instead, it should rely on its internal state to prevent
            // doing anything significant when dropped multiple times.
            drop(tv.x);

            // Here tv goes out of scope, tv.y should be dropped, but not tv.x.
        }

        assert_eq!(count_x, 1);
        assert_eq!(count_y, 1);
    }

    #[test]
    fn test_into_vec_bulk_relocation() {
        use alloc::rc::Rc;
        use core::cell::Cell;

        let empty = Vec::from(JackVec::<u64>::new());
        assert!(empty.is_empty());
        assert_eq!(empty.capacity(), 0);

        let allocated_empty = Vec::from(JackVec::<u64>::with_capacity(8));
        assert!(allocated_empty.is_empty());
        assert_eq!(allocated_empty.capacity(), 0);

        let mut source = JackVec::with_capacity(16);
        source.extend([10, 20, 30]);
        let values = Vec::from(source);
        assert_eq!(values, [10, 20, 30]);
        assert_eq!(values.capacity(), values.len());

        struct CountDrop(Rc<Cell<usize>>);
        impl Drop for CountDrop {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }

        let drops = Rc::new(Cell::new(0));
        let source = jack_vec![
            CountDrop(drops.clone()),
            CountDrop(drops.clone()),
            CountDrop(drops.clone()),
        ];
        let values = Vec::from(source);
        assert_eq!(drops.get(), 0);
        drop(values);
        assert_eq!(drops.get(), 3);

        #[derive(Debug, PartialEq)]
        struct Zst;
        impl Drop for Zst {
            fn drop(&mut self) {}
        }

        let zsts = Vec::from(jack_vec![Zst, Zst, Zst]);
        assert_eq!(zsts, [Zst, Zst, Zst]);
        {
            #[repr(align(64))]
            #[derive(Debug, PartialEq)]
            struct Aligned(u8);

            let aligned = Vec::from(jack_vec![Aligned(1), Aligned(2)]);
            assert_eq!(aligned, [Aligned(1), Aligned(2)]);
            assert_eq!(aligned.as_ptr().align_offset(64), 0);
        }
    }

    #[test]
    fn test_into_box_bulk_relocation() {
        use alloc::rc::Rc;
        use core::cell::Cell;

        assert!(Box::<[u64]>::from(JackVec::new()).is_empty());
        assert!(Box::<[u64]>::from(JackVec::with_capacity(8)).is_empty());

        let mut source = JackVec::with_capacity(16);
        source.extend([10, 20, 30]);
        let values = Box::<[u64]>::from(source);
        assert_eq!(&*values, [10, 20, 30]);

        struct CountDrop(Rc<Cell<usize>>);
        impl Drop for CountDrop {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }

        let drops = Rc::new(Cell::new(0));
        let source = jack_vec![
            CountDrop(drops.clone()),
            CountDrop(drops.clone()),
            CountDrop(drops.clone()),
        ];
        let values = Box::<[CountDrop]>::from(source);
        assert_eq!(drops.get(), 0);
        drop(values);
        assert_eq!(drops.get(), 3);

        #[derive(Debug, PartialEq)]
        struct Zst;
        impl Drop for Zst {
            fn drop(&mut self) {}
        }

        let zsts = Box::<[Zst]>::from(jack_vec![Zst, Zst, Zst]);
        assert_eq!(&*zsts, [Zst, Zst, Zst]);
        {
            #[repr(align(64))]
            #[derive(Debug, PartialEq)]
            struct Aligned(u8);

            let aligned = Box::<[Aligned]>::from(jack_vec![Aligned(1), Aligned(2)]);
            assert_eq!(&*aligned, [Aligned(1), Aligned(2)]);
            assert_eq!(aligned.as_ptr().align_offset(64), 0);
        }
    }

    #[test]
    fn test_from_vec_bulk_relocation() {
        use alloc::{rc::Rc, vec};
        use core::cell::Cell;

        assert!(JackVec::<u64>::from(Vec::new()).is_empty());
        assert!(JackVec::<u64>::from(Vec::with_capacity(8)).is_empty());

        let mut source = Vec::with_capacity(16);
        source.extend([10, 20, 30]);
        let values = JackVec::from(source);
        assert_eq!(values, [10, 20, 30]);
        assert_eq!(values.capacity(), values.len());

        struct CountDrop(Rc<Cell<usize>>);
        impl Drop for CountDrop {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }

        let drops = Rc::new(Cell::new(0));
        let source = vec![
            CountDrop(drops.clone()),
            CountDrop(drops.clone()),
            CountDrop(drops.clone()),
        ];
        let values = JackVec::from(source);
        assert_eq!(drops.get(), 0);
        drop(values);
        assert_eq!(drops.get(), 3);

        #[derive(Debug, PartialEq)]
        struct Zst;
        impl Drop for Zst {
            fn drop(&mut self) {}
        }

        let zsts = JackVec::from(vec![Zst, Zst, Zst]);
        assert_eq!(zsts, [Zst, Zst, Zst]);
        {
            #[repr(align(64))]
            #[derive(Debug, PartialEq)]
            struct Aligned(u8);

            let aligned = JackVec::from(vec![Aligned(1), Aligned(2)]);
            assert_eq!(aligned, [Aligned(1), Aligned(2)]);
            assert_eq!(aligned.as_ptr().align_offset(64), 0);
        }
    }

    #[test]
    fn test_from_array_bulk_relocation() {
        use alloc::rc::Rc;
        use core::cell::Cell;

        assert!(JackVec::<u64>::from([]).is_empty());
        assert_eq!(JackVec::from([10]), [10]);
        let values = JackVec::from([10, 20, 30, 40]);
        assert_eq!(values, [10, 20, 30, 40]);
        assert_eq!(values.capacity(), 4);

        struct CountDrop(Rc<Cell<usize>>);
        impl Drop for CountDrop {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }

        let drops = Rc::new(Cell::new(0));
        let values = JackVec::from([
            CountDrop(drops.clone()),
            CountDrop(drops.clone()),
            CountDrop(drops.clone()),
        ]);
        assert_eq!(drops.get(), 0);
        drop(values);
        assert_eq!(drops.get(), 3);

        #[derive(Debug, PartialEq)]
        struct Zst;
        let zsts = JackVec::from([Zst, Zst, Zst]);
        assert_eq!(zsts, [Zst, Zst, Zst]);
        {
            #[repr(align(64))]
            #[derive(Debug, PartialEq)]
            struct Aligned(u8);

            let aligned = JackVec::from([Aligned(1), Aligned(2)]);
            assert_eq!(aligned, [Aligned(1), Aligned(2)]);
            assert_eq!(aligned.as_ptr().align_offset(64), 0);
        }
    }

    #[test]
    fn test_reserve() {
        let mut v = JackVec::new();
        assert_eq!(v.capacity(), 0);

        v.reserve(2);
        assert!(v.capacity() >= 2);

        for i in 0..16 {
            v.push(i);
        }

        assert!(v.capacity() >= 16);
        v.reserve(16);
        assert!(v.capacity() >= 32);

        v.push(16);

        v.reserve(16);
        assert!(v.capacity() >= 33)
    }

    #[test]
    fn test_extend() {
        let mut v = JackVec::<usize>::new();
        let mut w = JackVec::new();
        v.extend(w.clone());
        assert_eq!(v, &[]);

        v.extend(0..3);
        for i in 0..3 {
            w.push(i)
        }

        assert_eq!(v, w);

        v.extend(3..10);
        for i in 3..10 {
            w.push(i)
        }

        assert_eq!(v, w);

        v.extend(w.clone()); // specializes to `append`
        assert!(v.iter().eq(w.iter().chain(w.iter())));

        // Zero sized types
        #[derive(PartialEq, Debug)]
        struct Foo;

        let mut a = JackVec::new();
        let b = jack_vec![Foo, Foo];

        a.extend(b);
        assert_eq!(a, &[Foo, Foo]);

        // Double drop
        let mut count_x = 0;
        {
            let mut x = JackVec::new();
            let y = jack_vec![DropCounter {
                count: &mut count_x
            }];
            x.extend(y);
        }

        assert_eq!(count_x, 1);
    }

    #[test]
    fn test_extend_size_hint_bounds() {
        struct Hint<I> {
            inner: I,
            lower: usize,
        }

        impl<I: Iterator> Iterator for Hint<I> {
            type Item = I::Item;

            fn next(&mut self) -> Option<Self::Item> {
                self.inner.next()
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                (self.lower, None)
            }
        }

        let mut overestimated = jack_vec![9];
        overestimated.extend(Hint {
            inner: 0..2,
            lower: 4,
        });
        assert_eq!(overestimated, [9, 0, 1]);

        let mut underestimated = jack_vec![9];
        underestimated.extend(Hint {
            inner: 0..4,
            lower: 2,
        });
        assert_eq!(underestimated, [9, 0, 1, 2, 3]);

        let mut empty = jack_vec![9];
        empty.extend(core::iter::empty());
        assert_eq!(empty, [9]);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_extend_iterator_panic_publishes_initialized_prefix() {
        use alloc::rc::Rc;
        use core::cell::Cell;
        use std::panic::{catch_unwind, AssertUnwindSafe};

        struct Counted {
            id: usize,
            drops: Rc<Vec<Cell<usize>>>,
        }

        impl Drop for Counted {
            fn drop(&mut self) {
                self.drops[self.id].set(self.drops[self.id].get() + 1);
            }
        }

        struct PanicIter {
            next: usize,
            drops: Rc<Vec<Cell<usize>>>,
        }

        impl Iterator for PanicIter {
            type Item = Counted;

            fn next(&mut self) -> Option<Self::Item> {
                if self.next == 2 {
                    panic!("iterator panic");
                }
                let id = self.next + 1;
                self.next += 1;
                Some(Counted {
                    id,
                    drops: self.drops.clone(),
                })
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                (4, Some(4))
            }
        }

        let drops = Rc::new((0..3).map(|_| Cell::new(0)).collect::<Vec<_>>());
        let mut values = jack_vec![Counted {
            id: 0,
            drops: drops.clone(),
        }];

        let result = catch_unwind(AssertUnwindSafe(|| {
            values.extend(PanicIter {
                next: 0,
                drops: drops.clone(),
            });
        }));

        assert!(result.is_err());
        assert_eq!(
            values.iter().map(|value| value.id).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        drop(values);
        assert!(drops.iter().all(|count| count.get() == 1));
    }

    #[test]
    fn test_resize() {
        let mut values = jack_vec![1, 2];
        values.resize(5, 9);
        assert_eq!(values, [1, 2, 9, 9, 9]);
        values.resize(5, 7);
        assert_eq!(values, [1, 2, 9, 9, 9]);
        values.resize(2, 0);
        assert_eq!(values, [1, 2]);

        let mut zst = JackVec::new();
        zst.resize(8, ());
        assert_eq!(zst.len(), 8);
        zst.resize(1, ());
        assert_eq!(zst.len(), 1);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_resize_clone_panic_publishes_initialized_suffix() {
        use alloc::rc::Rc;
        use core::cell::Cell;
        use std::panic::{catch_unwind, AssertUnwindSafe};

        #[derive(Clone, Copy)]
        enum Kind {
            Existing,
            Value,
            Clone,
        }

        struct PanicClone {
            kind: Kind,
            clone_attempts: Rc<Cell<usize>>,
            existing_drops: Rc<Cell<usize>>,
            value_drops: Rc<Cell<usize>>,
            clone_drops: Rc<Cell<usize>>,
        }

        impl Clone for PanicClone {
            fn clone(&self) -> Self {
                let attempt = self.clone_attempts.get() + 1;
                self.clone_attempts.set(attempt);
                if attempt == 3 {
                    panic!("clone panic");
                }
                Self {
                    kind: Kind::Clone,
                    clone_attempts: self.clone_attempts.clone(),
                    existing_drops: self.existing_drops.clone(),
                    value_drops: self.value_drops.clone(),
                    clone_drops: self.clone_drops.clone(),
                }
            }
        }

        impl Drop for PanicClone {
            fn drop(&mut self) {
                let count = match self.kind {
                    Kind::Existing => &self.existing_drops,
                    Kind::Value => &self.value_drops,
                    Kind::Clone => &self.clone_drops,
                };
                count.set(count.get() + 1);
            }
        }

        let clone_attempts = Rc::new(Cell::new(0));
        let existing_drops = Rc::new(Cell::new(0));
        let value_drops = Rc::new(Cell::new(0));
        let clone_drops = Rc::new(Cell::new(0));
        let make = |kind| PanicClone {
            kind,
            clone_attempts: clone_attempts.clone(),
            existing_drops: existing_drops.clone(),
            value_drops: value_drops.clone(),
            clone_drops: clone_drops.clone(),
        };
        let mut values = jack_vec![make(Kind::Existing)];

        let result = catch_unwind(AssertUnwindSafe(|| {
            values.resize(5, make(Kind::Value));
        }));

        assert!(result.is_err());
        assert_eq!(clone_attempts.get(), 3);
        assert_eq!(values.len(), 3);
        assert_eq!(existing_drops.get(), 0);
        assert_eq!(value_drops.get(), 1);
        assert_eq!(clone_drops.get(), 0);
        drop(values);
        assert_eq!(existing_drops.get(), 1);
        assert_eq!(value_drops.get(), 1);
        assert_eq!(clone_drops.get(), 2);
    }

    /* TODO: implement extend for Iter<&Copy>
        #[test]
        fn test_extend_ref() {
            let mut v = jack_vec![1, 2];
            v.extend(&[3, 4, 5]);

            assert_eq!(v.len(), 5);
            assert_eq!(v, [1, 2, 3, 4, 5]);

            let w = jack_vec![6, 7];
            v.extend(&w);

            assert_eq!(v.len(), 7);
            assert_eq!(v, [1, 2, 3, 4, 5, 6, 7]);
        }
    */

    #[test]
    fn test_slice_from_mut() {
        let mut values = jack_vec![1, 2, 3, 4, 5];
        {
            let slice = &mut values[2..];
            assert!(slice == [3, 4, 5]);
            for p in slice {
                *p += 2;
            }
        }

        assert!(values == [1, 2, 5, 6, 7]);
    }

    #[test]
    fn test_slice_to_mut() {
        let mut values = jack_vec![1, 2, 3, 4, 5];
        {
            let slice = &mut values[..2];
            assert!(slice == [1, 2]);
            for p in slice {
                *p += 1;
            }
        }

        assert!(values == [2, 3, 3, 4, 5]);
    }

    #[test]
    fn test_split_at_mut() {
        let mut values = jack_vec![1, 2, 3, 4, 5];
        {
            let (left, right) = values.split_at_mut(2);
            {
                let left: &[_] = left;
                assert!(left[..left.len()] == [1, 2]);
            }
            for p in left {
                *p += 1;
            }

            {
                let right: &[_] = right;
                assert!(right[..right.len()] == [3, 4, 5]);
            }
            for p in right {
                *p += 2;
            }
        }

        assert_eq!(values, [2, 3, 5, 6, 7]);
    }

    #[test]
    fn test_clone() {
        let v: JackVec<i32> = jack_vec![];
        let w = jack_vec![1, 2, 3];

        assert_eq!(v, v.clone());

        let z = w.clone();
        assert_eq!(w, z);
        // they should be disjoint in memory.
        assert!(w.as_ptr() != z.as_ptr())
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_clone_panic_drops_initialized_prefix() {
        use alloc::rc::Rc;
        use core::cell::Cell;
        use std::panic::{catch_unwind, AssertUnwindSafe};

        struct PanicClone {
            id: usize,
            is_clone: bool,
            original_drops: Rc<Vec<Cell<usize>>>,
            clone_drops: Rc<Vec<Cell<usize>>>,
        }

        impl Clone for PanicClone {
            fn clone(&self) -> Self {
                if self.id == 2 {
                    panic!("clone panic");
                }
                Self {
                    id: self.id,
                    is_clone: true,
                    original_drops: self.original_drops.clone(),
                    clone_drops: self.clone_drops.clone(),
                }
            }
        }

        impl Drop for PanicClone {
            fn drop(&mut self) {
                let drops = if self.is_clone {
                    &self.clone_drops
                } else {
                    &self.original_drops
                };
                drops[self.id].set(drops[self.id].get() + 1);
            }
        }

        let original_drops = Rc::new((0..4).map(|_| Cell::new(0)).collect::<Vec<_>>());
        let clone_drops = Rc::new((0..4).map(|_| Cell::new(0)).collect::<Vec<_>>());
        let source = (0..4)
            .map(|id| PanicClone {
                id,
                is_clone: false,
                original_drops: original_drops.clone(),
                clone_drops: clone_drops.clone(),
            })
            .collect::<JackVec<_>>();

        let result = catch_unwind(AssertUnwindSafe(|| source.clone()));

        assert!(result.is_err());
        assert!(original_drops.iter().all(|count| count.get() == 0));
        assert_eq!(
            clone_drops
                .iter()
                .map(|count| count.get())
                .collect::<Vec<_>>(),
            [1, 1, 0, 0]
        );
        drop(source);
        assert!(original_drops.iter().all(|count| count.get() == 1));
    }

    #[test]
    fn test_clone_from() {
        let mut v = jack_vec![];
        let three: JackVec<Box<_>> = jack_vec![Box::new(1), Box::new(2), Box::new(3)];
        let two: JackVec<Box<_>> = jack_vec![Box::new(4), Box::new(5)];
        // zero, long
        v.clone_from(&three);
        assert_eq!(v, three);

        // equal
        v.clone_from(&three);
        assert_eq!(v, three);

        // long, short
        v.clone_from(&two);
        assert_eq!(v, two);

        // short, long
        v.clone_from(&three);
        assert_eq!(v, three)
    }

    #[test]
    fn test_retain() {
        let mut vec = jack_vec![1, 2, 3, 4];
        vec.retain(|&x| x % 2 == 0);
        assert_eq!(vec, [2, 4]);
    }

    #[test]
    fn test_retain_mut() {
        let mut vec = jack_vec![9, 9, 9, 9];
        let mut i = 0;
        vec.retain_mut(|x| {
            i += 1;
            *x = i;
            i != 4
        });
        assert_eq!(vec, [1, 2, 3]);
    }

    #[test]
    fn test_retain_mut_edge_cases() {
        let mut empty = JackVec::<()>::new();
        empty.retain_mut(|_| unreachable!());

        let mut zst = jack_vec![(); 8];
        let mut index = 0;
        zst.retain_mut(|_| {
            let keep = index % 2 == 0;
            index += 1;
            keep
        });
        assert_eq!(zst.len(), 4);

        let mut values = jack_vec![1, 2, 3, 4];
        values.retain_mut(|_| true);
        assert_eq!(values, [1, 2, 3, 4]);
        values.retain_mut(|_| false);
        assert!(values.is_empty());
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_retain_mut_predicate_panic_repairs_hole() {
        use alloc::rc::Rc;
        use core::cell::Cell;
        use std::panic::{catch_unwind, AssertUnwindSafe};

        struct Counted {
            id: usize,
            drops: Rc<Vec<Cell<usize>>>,
        }

        impl Drop for Counted {
            fn drop(&mut self) {
                self.drops[self.id].set(self.drops[self.id].get() + 1);
            }
        }

        let drops = Rc::new((0..6).map(|_| Cell::new(0)).collect::<Vec<_>>());
        let mut values = (0..6)
            .map(|id| Counted {
                id,
                drops: drops.clone(),
            })
            .collect::<JackVec<_>>();

        let result = catch_unwind(AssertUnwindSafe(|| {
            values.retain_mut(|value| match value.id {
                1 => false,
                3 => panic!("predicate panic"),
                _ => true,
            });
        }));

        assert!(result.is_err());
        assert_eq!(
            values.iter().map(|value| value.id).collect::<Vec<_>>(),
            [0, 2, 3, 4, 5]
        );
        assert_eq!(drops[1].get(), 1);
        drop(values);
        assert!(drops.iter().all(|count| count.get() == 1));
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_retain_mut_rejected_drop_panic_repairs_hole() {
        use alloc::rc::Rc;
        use core::cell::Cell;
        use std::panic::{catch_unwind, AssertUnwindSafe};

        struct PanicDrop {
            id: usize,
            drops: Rc<Vec<Cell<usize>>>,
        }

        impl Drop for PanicDrop {
            fn drop(&mut self) {
                let old_count = self.drops[self.id].replace(self.drops[self.id].get() + 1);
                if self.id == 1 && old_count == 0 {
                    panic!("drop panic");
                }
            }
        }

        let drops = Rc::new((0..5).map(|_| Cell::new(0)).collect::<Vec<_>>());
        let mut values = (0..5)
            .map(|id| PanicDrop {
                id,
                drops: drops.clone(),
            })
            .collect::<JackVec<_>>();

        let result = catch_unwind(AssertUnwindSafe(|| {
            values.retain_mut(|value| value.id != 1);
        }));

        assert!(result.is_err());
        assert_eq!(
            values.iter().map(|value| value.id).collect::<Vec<_>>(),
            [0, 2, 3, 4]
        );
        assert_eq!(drops[1].get(), 1);
        drop(values);
        assert!(drops.iter().all(|count| count.get() == 1));
    }

    #[test]
    fn test_dedup() {
        fn case(a: JackVec<i32>, b: JackVec<i32>) {
            let mut v = a;
            v.dedup();
            assert_eq!(v, b);
        }
        case(jack_vec![], jack_vec![]);
        case(jack_vec![1], jack_vec![1]);
        case(jack_vec![1, 1], jack_vec![1]);
        case(jack_vec![1, 2, 3], jack_vec![1, 2, 3]);
        case(jack_vec![1, 1, 2, 3], jack_vec![1, 2, 3]);
        case(jack_vec![1, 2, 2, 3], jack_vec![1, 2, 3]);
        case(jack_vec![1, 2, 3, 3], jack_vec![1, 2, 3]);
        case(jack_vec![1, 1, 2, 2, 2, 3, 3], jack_vec![1, 2, 3]);
    }

    #[test]
    fn test_dedup_by_key() {
        fn case(a: JackVec<i32>, b: JackVec<i32>) {
            let mut v = a;
            v.dedup_by_key(|i| *i / 10);
            assert_eq!(v, b);
        }
        case(jack_vec![], jack_vec![]);
        case(jack_vec![10], jack_vec![10]);
        case(jack_vec![10, 11], jack_vec![10]);
        case(jack_vec![10, 20, 30], jack_vec![10, 20, 30]);
        case(jack_vec![10, 11, 20, 30], jack_vec![10, 20, 30]);
        case(jack_vec![10, 20, 21, 30], jack_vec![10, 20, 30]);
        case(jack_vec![10, 20, 30, 31], jack_vec![10, 20, 30]);
        case(jack_vec![10, 11, 20, 21, 22, 30, 31], jack_vec![10, 20, 30]);
    }

    #[test]
    fn test_dedup_by() {
        let mut vec = jack_vec!["foo", "bar", "Bar", "baz", "bar"];
        vec.dedup_by(|a, b| a.eq_ignore_ascii_case(b));

        assert_eq!(vec, ["foo", "bar", "baz", "bar"]);

        let mut vec = jack_vec![("foo", 1), ("foo", 2), ("bar", 3), ("bar", 4), ("bar", 5)];
        vec.dedup_by(|a, b| {
            a.0 == b.0 && {
                b.1 += a.1;
                true
            }
        });

        assert_eq!(vec, [("foo", 3), ("bar", 12)]);
    }

    #[test]
    fn test_dedup_by_edge_cases() {
        let mut empty = JackVec::<()>::new();
        empty.dedup_by(|_, _| unreachable!());

        let mut singleton = jack_vec![1];
        singleton.dedup_by(|_, _| unreachable!());
        assert_eq!(singleton, [1]);

        let mut unique = jack_vec![1, 2, 3, 4];
        unique.dedup_by(|current, previous| current == previous);
        assert_eq!(unique, [1, 2, 3, 4]);

        let mut duplicate = jack_vec![1, 1, 1, 1];
        duplicate.dedup_by(|current, previous| current == previous);
        assert_eq!(duplicate, [1]);

        let mut zst = jack_vec![(); 8];
        let mut comparisons = 0;
        zst.dedup_by(|_, _| {
            comparisons += 1;
            true
        });
        assert_eq!(zst.len(), 1);
        assert_eq!(comparisons, 7);

        // Exercise the post-gap survivor copy for a ZST, whose source and
        // destination have the same numerical address.
        let mut zst_mixed = jack_vec![(); 8];
        let mut comparisons = 0;
        zst_mixed.dedup_by(|_, _| {
            comparisons += 1;
            comparisons % 2 == 1
        });
        assert_eq!(zst_mixed.len(), 4);
        assert_eq!(comparisons, 7);
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_dedup_by_comparator_panic_repairs_gap() {
        use alloc::rc::Rc;
        use core::cell::Cell;
        use std::panic::{catch_unwind, AssertUnwindSafe};

        struct Counted {
            id: usize,
            drops: Rc<Vec<Cell<usize>>>,
        }

        impl Drop for Counted {
            fn drop(&mut self) {
                self.drops[self.id].set(self.drops[self.id].get() + 1);
            }
        }

        let drops = Rc::new((0..6).map(|_| Cell::new(0)).collect::<Vec<_>>());
        let mut values = (0..6)
            .map(|id| Counted {
                id,
                drops: drops.clone(),
            })
            .collect::<JackVec<_>>();

        let result = catch_unwind(AssertUnwindSafe(|| {
            values.dedup_by(|current, previous| {
                if current.id == 4 {
                    panic!("comparator panic");
                }
                current.id / 2 == previous.id / 2
            });
        }));

        assert!(result.is_err());
        assert_eq!(
            values.iter().map(|value| value.id).collect::<Vec<_>>(),
            [0, 2, 4, 5]
        );
        assert_eq!(drops[1].get(), 1);
        assert_eq!(drops[3].get(), 1);
        drop(values);
        assert!(drops.iter().all(|count| count.get() == 1));
    }

    #[cfg(feature = "std")]
    #[test]
    fn test_dedup_by_duplicate_drop_panic_repairs_gap() {
        use alloc::rc::Rc;
        use core::cell::Cell;
        use std::panic::{catch_unwind, AssertUnwindSafe};

        struct PanicDrop {
            id: usize,
            drops: Rc<Vec<Cell<usize>>>,
        }

        impl Drop for PanicDrop {
            fn drop(&mut self) {
                let old_count = self.drops[self.id].replace(self.drops[self.id].get() + 1);
                if self.id == 1 && old_count == 0 {
                    panic!("drop panic");
                }
            }
        }

        let drops = Rc::new((0..5).map(|_| Cell::new(0)).collect::<Vec<_>>());
        let mut values = (0..5)
            .map(|id| PanicDrop {
                id,
                drops: drops.clone(),
            })
            .collect::<JackVec<_>>();

        let result = catch_unwind(AssertUnwindSafe(|| {
            values.dedup_by(|current, previous| current.id / 2 == previous.id / 2);
        }));

        assert!(result.is_err());
        assert_eq!(
            values.iter().map(|value| value.id).collect::<Vec<_>>(),
            [0, 2, 3, 4]
        );
        assert_eq!(drops[1].get(), 1);
        drop(values);
        assert!(drops.iter().all(|count| count.get() == 1));
    }

    #[test]
    fn test_dedup_unique() {
        let mut v0: JackVec<Box<_>> = jack_vec![Box::new(1), Box::new(1), Box::new(2), Box::new(3)];
        v0.dedup();
        let mut v1: JackVec<Box<_>> = jack_vec![Box::new(1), Box::new(2), Box::new(2), Box::new(3)];
        v1.dedup();
        let mut v2: JackVec<Box<_>> = jack_vec![Box::new(1), Box::new(2), Box::new(3), Box::new(3)];
        v2.dedup();
        // If the boxed pointers were leaked or otherwise misused, valgrind
        // and/or rt should raise errors.
    }

    #[test]
    fn zero_sized_values() {
        let mut v = JackVec::new();
        assert_eq!(v.len(), 0);
        v.push(());
        assert_eq!(v.len(), 1);
        v.push(());
        assert_eq!(v.len(), 2);
        assert_eq!(v.pop(), Some(()));
        assert_eq!(v.pop(), Some(()));
        assert_eq!(v.pop(), None);

        assert_eq!(v.iter().count(), 0);
        v.push(());
        assert_eq!(v.iter().count(), 1);
        v.push(());
        assert_eq!(v.iter().count(), 2);

        for &() in &v {}

        assert_eq!(v.iter_mut().count(), 2);
        v.push(());
        assert_eq!(v.iter_mut().count(), 3);
        v.push(());
        assert_eq!(v.iter_mut().count(), 4);

        for &mut () in &mut v {}
        unsafe {
            v.set_len(0);
        }
        assert_eq!(v.iter_mut().count(), 0);
    }

    #[test]
    fn test_partition() {
        assert_eq!(
            jack_vec![].into_iter().partition(|x: &i32| *x < 3),
            (jack_vec![], jack_vec![])
        );
        assert_eq!(
            jack_vec![1, 2, 3].into_iter().partition(|x| *x < 4),
            (jack_vec![1, 2, 3], jack_vec![])
        );
        assert_eq!(
            jack_vec![1, 2, 3].into_iter().partition(|x| *x < 2),
            (jack_vec![1], jack_vec![2, 3])
        );
        assert_eq!(
            jack_vec![1, 2, 3].into_iter().partition(|x| *x < 0),
            (jack_vec![], jack_vec![1, 2, 3])
        );
    }

    #[test]
    fn test_zip_unzip() {
        let z1 = jack_vec![(1, 4), (2, 5), (3, 6)];

        let (left, right): (JackVec<_>, JackVec<_>) = z1.iter().cloned().unzip();

        assert_eq!((1, 4), (left[0], right[0]));
        assert_eq!((2, 5), (left[1], right[1]));
        assert_eq!((3, 6), (left[2], right[2]));
    }

    #[test]
    fn test_vec_truncate_drop() {
        static mut DROPS: u32 = 0;
        #[allow(unused)]
        struct Elem(i32);
        impl Drop for Elem {
            fn drop(&mut self) {
                unsafe {
                    DROPS += 1;
                }
            }
        }

        let mut v = jack_vec![Elem(1), Elem(2), Elem(3), Elem(4), Elem(5)];
        assert_eq!(unsafe { DROPS }, 0);
        v.truncate(3);
        assert_eq!(unsafe { DROPS }, 2);
        v.truncate(0);
        assert_eq!(unsafe { DROPS }, 5);
    }

    #[test]
    #[should_panic]
    fn test_vec_truncate_fail() {
        struct BadElem(i32);
        impl Drop for BadElem {
            fn drop(&mut self) {
                let BadElem(ref mut x) = *self;
                if *x == 0xbadbeef {
                    panic!("BadElem panic: 0xbadbeef")
                }
            }
        }

        let mut v = jack_vec![BadElem(1), BadElem(2), BadElem(0xbadbeef), BadElem(4)];
        v.truncate(0);
    }

    #[test]
    fn test_index() {
        let vec = jack_vec![1, 2, 3];
        assert!(vec[1] == 2);
    }

    #[test]
    #[should_panic]
    fn test_index_out_of_bounds() {
        let vec = jack_vec![1, 2, 3];
        let _ = vec[3];
    }

    #[test]
    #[should_panic]
    fn test_slice_out_of_bounds_1() {
        let x = jack_vec![1, 2, 3, 4, 5];
        let _ = &x[!0..];
    }

    #[test]
    #[should_panic]
    fn test_slice_out_of_bounds_2() {
        let x = jack_vec![1, 2, 3, 4, 5];
        let _ = &x[..6];
    }

    #[test]
    #[should_panic]
    fn test_slice_out_of_bounds_3() {
        let x = jack_vec![1, 2, 3, 4, 5];
        let _ = &x[!0..4];
    }

    #[test]
    #[should_panic]
    fn test_slice_out_of_bounds_4() {
        let x = jack_vec![1, 2, 3, 4, 5];
        let _ = &x[1..6];
    }

    #[test]
    #[should_panic]
    fn test_slice_out_of_bounds_5() {
        let x = jack_vec![1, 2, 3, 4, 5];
        let _ = &x[3..2];
    }

    #[test]
    #[should_panic]
    fn test_swap_remove_empty() {
        let mut vec = JackVec::<i32>::new();
        vec.swap_remove(0);
    }

    #[test]
    fn test_move_items() {
        let vec = jack_vec![1, 2, 3];
        let mut vec2 = jack_vec![];
        for i in vec {
            vec2.push(i);
        }
        assert_eq!(vec2, [1, 2, 3]);
    }

    #[test]
    fn test_move_items_reverse() {
        let vec = jack_vec![1, 2, 3];
        let mut vec2 = jack_vec![];
        for i in vec.into_iter().rev() {
            vec2.push(i);
        }
        assert_eq!(vec2, [3, 2, 1]);
    }

    #[test]
    fn test_move_items_zero_sized() {
        let vec = jack_vec![(), (), ()];
        let mut vec2 = jack_vec![];
        for i in vec {
            vec2.push(i);
        }
        assert_eq!(vec2, [(), (), ()]);
    }

    #[test]
    fn test_drain_items() {
        let mut vec = jack_vec![1, 2, 3];
        let mut vec2 = jack_vec![];
        for i in vec.drain(..) {
            vec2.push(i);
        }
        assert_eq!(vec, []);
        assert_eq!(vec2, [1, 2, 3]);
    }

    #[test]
    fn test_drain_items_reverse() {
        let mut vec = jack_vec![1, 2, 3];
        let mut vec2 = jack_vec![];
        for i in vec.drain(..).rev() {
            vec2.push(i);
        }
        assert_eq!(vec, []);
        assert_eq!(vec2, [3, 2, 1]);
    }

    #[test]
    fn test_drain_items_zero_sized() {
        let mut vec = jack_vec![(), (), ()];
        let mut vec2 = jack_vec![];
        for i in vec.drain(..) {
            vec2.push(i);
        }
        assert_eq!(vec, []);
        assert_eq!(vec2, [(), (), ()]);
    }

    #[test]
    #[should_panic]
    fn test_drain_out_of_bounds() {
        let mut v = jack_vec![1, 2, 3, 4, 5];
        v.drain(5..6);
    }

    #[test]
    fn test_drain_range() {
        let mut v = jack_vec![1, 2, 3, 4, 5];
        for _ in v.drain(4..) {}
        assert_eq!(v, &[1, 2, 3, 4]);

        let mut v: JackVec<_> = (1..6).map(|x| x.to_string()).collect();
        for _ in v.drain(1..4) {}
        assert_eq!(v, &[1.to_string(), 5.to_string()]);

        let mut v: JackVec<_> = (1..6).map(|x| x.to_string()).collect();
        for _ in v.drain(1..4).rev() {}
        assert_eq!(v, &[1.to_string(), 5.to_string()]);

        let mut v: JackVec<_> = jack_vec![(); 5];
        for _ in v.drain(1..4).rev() {}
        assert_eq!(v, &[(), ()]);
    }

    #[test]
    fn test_drain_inclusive_range() {
        let mut v = jack_vec!['a', 'b', 'c', 'd', 'e'];
        for _ in v.drain(1..=3) {}
        assert_eq!(v, &['a', 'e']);

        let mut v: JackVec<_> = (0..=5).map(|x| x.to_string()).collect();
        for _ in v.drain(1..=5) {}
        assert_eq!(v, &["0".to_string()]);

        let mut v: JackVec<String> = (0..=5).map(|x| x.to_string()).collect();
        for _ in v.drain(0..=5) {}
        assert_eq!(v, JackVec::<String>::new());

        let mut v: JackVec<_> = (0..=5).map(|x| x.to_string()).collect();
        for _ in v.drain(0..=3) {}
        assert_eq!(v, &["4".to_string(), "5".to_string()]);

        let mut v: JackVec<_> = (0..=1).map(|x| x.to_string()).collect();
        for _ in v.drain(..=0) {}
        assert_eq!(v, &["1".to_string()]);
    }

    #[test]
    fn test_drain_max_vec_size() {
        let mut v = JackVec::<()>::with_capacity(MAX_CAP);
        unsafe {
            v.set_len(MAX_CAP);
        }
        for _ in v.drain(MAX_CAP - 1..) {}
        assert_eq!(v.len(), MAX_CAP - 1);

        let mut v = JackVec::<()>::with_capacity(MAX_CAP);
        unsafe {
            v.set_len(MAX_CAP);
        }
        for _ in v.drain(MAX_CAP - 1..=MAX_CAP - 1) {}
        assert_eq!(v.len(), MAX_CAP - 1);
    }

    #[test]
    #[should_panic]
    fn test_drain_inclusive_out_of_bounds() {
        let mut v = jack_vec![1, 2, 3, 4, 5];
        v.drain(5..=5);
    }

    #[test]
    fn test_splice() {
        let mut v = jack_vec![1, 2, 3, 4, 5];
        let a = [10, 11, 12];
        v.splice(2..4, a.iter().cloned());
        assert_eq!(v, &[1, 2, 10, 11, 12, 5]);
        v.splice(1..3, Some(20));
        assert_eq!(v, &[1, 20, 11, 12, 5]);
    }

    #[test]
    fn test_splice_reserves_only_required_capacity() {
        let mut v = JackVec::with_capacity(5);
        v.extend(0..5);

        drop(v.splice(2..4, 10..21));

        assert_eq!(v, &[0, 1, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 4]);
        assert_eq!(v.capacity(), 14);
    }

    #[test]
    fn test_splice_inclusive_range() {
        let mut v = jack_vec![1, 2, 3, 4, 5];
        let a = [10, 11, 12];
        let t1: JackVec<_> = v.splice(2..=3, a.iter().cloned()).collect();
        assert_eq!(v, &[1, 2, 10, 11, 12, 5]);
        assert_eq!(t1, &[3, 4]);
        let t2: JackVec<_> = v.splice(1..=2, Some(20)).collect();
        assert_eq!(v, &[1, 20, 11, 12, 5]);
        assert_eq!(t2, &[2, 10]);
    }

    #[test]
    #[should_panic]
    fn test_splice_out_of_bounds() {
        let mut v = jack_vec![1, 2, 3, 4, 5];
        let a = [10, 11, 12];
        v.splice(5..6, a.iter().cloned());
    }

    #[test]
    #[should_panic]
    fn test_splice_inclusive_out_of_bounds() {
        let mut v = jack_vec![1, 2, 3, 4, 5];
        let a = [10, 11, 12];
        v.splice(5..=5, a.iter().cloned());
    }

    #[test]
    fn test_splice_items_zero_sized() {
        let mut vec = jack_vec![(), (), ()];
        let vec2 = jack_vec![];
        let t: JackVec<_> = vec.splice(1..2, vec2.iter().cloned()).collect();
        assert_eq!(vec, &[(), ()]);
        assert_eq!(t, &[()]);
    }

    #[test]
    fn test_splice_unbounded() {
        let mut vec = jack_vec![1, 2, 3, 4, 5];
        let t: JackVec<_> = vec.splice(.., None).collect();
        assert_eq!(vec, &[]);
        assert_eq!(t, &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_splice_forget() {
        let mut v = jack_vec![1, 2, 3, 4, 5];
        let a = [10, 11, 12];
        ::core::mem::forget(v.splice(2..4, a.iter().cloned()));
        assert_eq!(v, &[1, 2]);
    }

    #[test]
    fn test_splice_from_empty() {
        let mut v = jack_vec![];
        let a = [10, 11, 12];
        v.splice(.., a.iter().cloned());
        assert_eq!(v, &[10, 11, 12]);
    }

    /* probs won't ever impl this
        #[test]
        fn test_into_boxed_slice() {
            let xs = jack_vec![1, 2, 3];
            let ys = xs.into_boxed_slice();
            assert_eq!(&*ys, [1, 2, 3]);
        }
    */

    #[test]
    fn test_append() {
        let mut vec = jack_vec![1, 2, 3];
        let mut vec2 = jack_vec![4, 5, 6];
        vec.append(&mut vec2);
        assert_eq!(vec, [1, 2, 3, 4, 5, 6]);
        assert_eq!(vec2, []);
    }

    #[test]
    fn test_append_drop_and_zst() {
        use alloc::rc::Rc;
        use core::cell::Cell;

        struct CountDrop(Rc<Cell<usize>>);
        impl Drop for CountDrop {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }

        let drops = Rc::new(Cell::new(0));
        {
            let mut destination = jack_vec![CountDrop(drops.clone()), CountDrop(drops.clone())];
            let mut source = jack_vec![
                CountDrop(drops.clone()),
                CountDrop(drops.clone()),
                CountDrop(drops.clone()),
            ];
            destination.append(&mut source);
            assert_eq!(destination.len(), 5);
            assert!(source.is_empty());
            assert_eq!(drops.get(), 0);
        }
        assert_eq!(drops.get(), 5);

        let mut destination = jack_vec![(), ()];
        let mut source = jack_vec![(), (), ()];
        destination.append(&mut source);
        assert_eq!(destination.len(), 5);
        assert!(source.is_empty());
    }

    #[test]
    fn test_split_off() {
        let mut vec = jack_vec![1, 2, 3, 4, 5, 6];
        let vec2 = vec.split_off(4);
        assert_eq!(vec, [1, 2, 3, 4]);
        assert_eq!(vec2, [5, 6]);
    }

    #[test]
    fn test_into_iter_as_slice() {
        let vec = jack_vec!['a', 'b', 'c'];
        let mut into_iter = vec.into_iter();
        assert_eq!(into_iter.as_slice(), &['a', 'b', 'c']);
        let _ = into_iter.next().unwrap();
        assert_eq!(into_iter.as_slice(), &['b', 'c']);
        let _ = into_iter.next().unwrap();
        let _ = into_iter.next().unwrap();
        assert_eq!(into_iter.as_slice(), &[]);
    }

    #[test]
    fn test_into_iter_as_mut_slice() {
        let vec = jack_vec!['a', 'b', 'c'];
        let mut into_iter = vec.into_iter();
        assert_eq!(into_iter.as_slice(), &['a', 'b', 'c']);
        into_iter.as_mut_slice()[0] = 'x';
        into_iter.as_mut_slice()[1] = 'y';
        assert_eq!(into_iter.next().unwrap(), 'x');
        assert_eq!(into_iter.as_slice(), &['y', 'c']);
    }

    #[test]
    fn test_into_iter_debug() {
        let vec = jack_vec!['a', 'b', 'c'];
        let into_iter = vec.into_iter();
        let debug = format!("{:?}", into_iter);
        assert_eq!(debug, "IntoIter(['a', 'b', 'c'])");
    }

    #[test]
    fn test_into_iter_count() {
        assert_eq!(jack_vec![1, 2, 3].into_iter().count(), 3);
    }

    #[test]
    fn test_into_iter_clone() {
        fn iter_equal<I: Iterator<Item = i32>>(it: I, slice: &[i32]) {
            let v: JackVec<i32> = it.collect();
            assert_eq!(&v[..], slice);
        }
        let mut it = jack_vec![1, 2, 3].into_iter();
        iter_equal(it.clone(), &[1, 2, 3]);
        assert_eq!(it.next(), Some(1));
        let mut it = it.rev();
        iter_equal(it.clone(), &[3, 2]);
        assert_eq!(it.next(), Some(3));
        iter_equal(it.clone(), &[2]);
        assert_eq!(it.next(), Some(2));
        iter_equal(it.clone(), &[]);
        assert_eq!(it.next(), None);
    }

    #[allow(dead_code)]
    fn assert_covariance() {
        fn drain<'new>(d: Drain<'static, &'static str>) -> Drain<'new, &'new str> {
            d
        }
        fn into_iter<'new>(i: IntoIter<&'static str>) -> IntoIter<&'new str> {
            i
        }
    }

    /* TODO: specialize vec.into_iter().collect::<JackVec<_>>();
        #[test]
        fn from_into_inner() {
            let vec = jack_vec![1, 2, 3];
            let ptr = vec.as_ptr();
            let vec = vec.into_iter().collect::<JackVec<_>>();
            assert_eq!(vec, [1, 2, 3]);
            assert_eq!(vec.as_ptr(), ptr);

            let ptr = &vec[1] as *const _;
            let mut it = vec.into_iter();
            it.next().unwrap();
            let vec = it.collect::<JackVec<_>>();
            assert_eq!(vec, [2, 3]);
            assert!(ptr != vec.as_ptr());
        }
    */

    #[test]
    fn overaligned_allocations() {
        #[repr(align(256))]
        struct Foo(usize);
        let mut v = jack_vec![Foo(273)];
        for i in 0..0x1000 {
            v.reserve_exact(i);
            assert!(v[0].0 == 273);
            assert!(v.as_ptr() as usize & 0xff == 0);
            v.shrink_to_fit();
            assert!(v[0].0 == 273);
            assert!(v.as_ptr() as usize & 0xff == 0);
        }
    }

    /* TODO: implement drain_filter?
        #[test]
        fn drain_filter_empty() {
            let mut vec: JackVec<i32> = jack_vec![];

            {
                let mut iter = vec.drain_filter(|_| true);
                assert_eq!(iter.size_hint(), (0, Some(0)));
                assert_eq!(iter.next(), None);
                assert_eq!(iter.size_hint(), (0, Some(0)));
                assert_eq!(iter.next(), None);
                assert_eq!(iter.size_hint(), (0, Some(0)));
            }
            assert_eq!(vec.len(), 0);
            assert_eq!(vec, jack_vec![]);
        }

        #[test]
        fn drain_filter_zst() {
            let mut vec = jack_vec![(), (), (), (), ()];
            let initial_len = vec.len();
            let mut count = 0;
            {
                let mut iter = vec.drain_filter(|_| true);
                assert_eq!(iter.size_hint(), (0, Some(initial_len)));
                while let Some(_) = iter.next() {
                    count += 1;
                    assert_eq!(iter.size_hint(), (0, Some(initial_len - count)));
                }
                assert_eq!(iter.size_hint(), (0, Some(0)));
                assert_eq!(iter.next(), None);
                assert_eq!(iter.size_hint(), (0, Some(0)));
            }

            assert_eq!(count, initial_len);
            assert_eq!(vec.len(), 0);
            assert_eq!(vec, jack_vec![]);
        }

        #[test]
        fn drain_filter_false() {
            let mut vec = jack_vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

            let initial_len = vec.len();
            let mut count = 0;
            {
                let mut iter = vec.drain_filter(|_| false);
                assert_eq!(iter.size_hint(), (0, Some(initial_len)));
                for _ in iter.by_ref() {
                    count += 1;
                }
                assert_eq!(iter.size_hint(), (0, Some(0)));
                assert_eq!(iter.next(), None);
                assert_eq!(iter.size_hint(), (0, Some(0)));
            }

            assert_eq!(count, 0);
            assert_eq!(vec.len(), initial_len);
            assert_eq!(vec, jack_vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        }

        #[test]
        fn drain_filter_true() {
            let mut vec = jack_vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

            let initial_len = vec.len();
            let mut count = 0;
            {
                let mut iter = vec.drain_filter(|_| true);
                assert_eq!(iter.size_hint(), (0, Some(initial_len)));
                while let Some(_) = iter.next() {
                    count += 1;
                    assert_eq!(iter.size_hint(), (0, Some(initial_len - count)));
                }
                assert_eq!(iter.size_hint(), (0, Some(0)));
                assert_eq!(iter.next(), None);
                assert_eq!(iter.size_hint(), (0, Some(0)));
            }

            assert_eq!(count, initial_len);
            assert_eq!(vec.len(), 0);
            assert_eq!(vec, jack_vec![]);
        }

        #[test]
        fn drain_filter_complex() {

            {   //                [+xxx++++++xxxxx++++x+x++]
                let mut vec = jack_vec![1,
                                   2, 4, 6,
                                   7, 9, 11, 13, 15, 17,
                                   18, 20, 22, 24, 26,
                                   27, 29, 31, 33,
                                   34,
                                   35,
                                   36,
                                   37, 39];

                let removed = vec.drain_filter(|x| *x % 2 == 0).collect::<JackVec<_>>();
                assert_eq!(removed.len(), 10);
                assert_eq!(removed, jack_vec![2, 4, 6, 18, 20, 22, 24, 26, 34, 36]);

                assert_eq!(vec.len(), 14);
                assert_eq!(vec, jack_vec![1, 7, 9, 11, 13, 15, 17, 27, 29, 31, 33, 35, 37, 39]);
            }

            {   //                [xxx++++++xxxxx++++x+x++]
                let mut vec = jack_vec![2, 4, 6,
                                   7, 9, 11, 13, 15, 17,
                                   18, 20, 22, 24, 26,
                                   27, 29, 31, 33,
                                   34,
                                   35,
                                   36,
                                   37, 39];

                let removed = vec.drain_filter(|x| *x % 2 == 0).collect::<JackVec<_>>();
                assert_eq!(removed.len(), 10);
                assert_eq!(removed, jack_vec![2, 4, 6, 18, 20, 22, 24, 26, 34, 36]);

                assert_eq!(vec.len(), 13);
                assert_eq!(vec, jack_vec![7, 9, 11, 13, 15, 17, 27, 29, 31, 33, 35, 37, 39]);
            }

            {   //                [xxx++++++xxxxx++++x+x]
                let mut vec = jack_vec![2, 4, 6,
                                   7, 9, 11, 13, 15, 17,
                                   18, 20, 22, 24, 26,
                                   27, 29, 31, 33,
                                   34,
                                   35,
                                   36];

                let removed = vec.drain_filter(|x| *x % 2 == 0).collect::<JackVec<_>>();
                assert_eq!(removed.len(), 10);
                assert_eq!(removed, jack_vec![2, 4, 6, 18, 20, 22, 24, 26, 34, 36]);

                assert_eq!(vec.len(), 11);
                assert_eq!(vec, jack_vec![7, 9, 11, 13, 15, 17, 27, 29, 31, 33, 35]);
            }

            {   //                [xxxxxxxxxx+++++++++++]
                let mut vec = jack_vec![2, 4, 6, 8, 10, 12, 14, 16, 18, 20,
                                   1, 3, 5, 7, 9, 11, 13, 15, 17, 19];

                let removed = vec.drain_filter(|x| *x % 2 == 0).collect::<JackVec<_>>();
                assert_eq!(removed.len(), 10);
                assert_eq!(removed, jack_vec![2, 4, 6, 8, 10, 12, 14, 16, 18, 20]);

                assert_eq!(vec.len(), 10);
                assert_eq!(vec, jack_vec![1, 3, 5, 7, 9, 11, 13, 15, 17, 19]);
            }

            {   //                [+++++++++++xxxxxxxxxx]
                let mut vec = jack_vec![1, 3, 5, 7, 9, 11, 13, 15, 17, 19,
                                   2, 4, 6, 8, 10, 12, 14, 16, 18, 20];

                let removed = vec.drain_filter(|x| *x % 2 == 0).collect::<JackVec<_>>();
                assert_eq!(removed.len(), 10);
                assert_eq!(removed, jack_vec![2, 4, 6, 8, 10, 12, 14, 16, 18, 20]);

                assert_eq!(vec.len(), 10);
                assert_eq!(vec, jack_vec![1, 3, 5, 7, 9, 11, 13, 15, 17, 19]);
            }
        }
    */
    #[test]
    fn test_reserve_exact() {
        // This is all the same as test_reserve

        let mut v = JackVec::new();
        assert_eq!(v.capacity(), 0);

        v.reserve_exact(2);
        assert!(v.capacity() >= 2);

        for i in 0..16 {
            v.push(i);
        }

        assert!(v.capacity() >= 16);
        v.reserve_exact(16);
        assert!(v.capacity() >= 32);

        v.push(16);

        v.reserve_exact(16);
        assert!(v.capacity() >= 33)
    }

    /* TODO: implement try_reserve
        #[test]
        fn test_try_reserve() {

            // These are the interesting cases:
            // * exactly isize::MAX should never trigger a CapacityOverflow (can be OOM)
            // * > isize::MAX should always fail
            //    * On 16/32-bit should CapacityOverflow
            //    * On 64-bit should OOM
            // * overflow may trigger when adding `len` to `cap` (in number of elements)
            // * overflow may trigger when multiplying `new_cap` by size_of::<T> (to get bytes)

            const MAX_CAP: usize = isize::MAX as usize;
            const MAX_USIZE: usize = usize::MAX;

            // On 16/32-bit, we check that allocations don't exceed isize::MAX,
            // on 64-bit, we assume the OS will give an OOM for such a ridiculous size.
            // Any platform that succeeds for these requests is technically broken with
            // ptr::offset because LLVM is the worst.
            let guards_against_isize = size_of::<usize>() < 8;

            {
                // Note: basic stuff is checked by test_reserve
                let mut empty_bytes: JackVec<u8> = JackVec::new();

                // Check isize::MAX doesn't count as an overflow
                if let Err(CapacityOverflow) = empty_bytes.try_reserve(MAX_CAP) {
                    panic!("isize::MAX shouldn't trigger an overflow!");
                }
                // Play it again, frank! (just to be sure)
                if let Err(CapacityOverflow) = empty_bytes.try_reserve(MAX_CAP) {
                    panic!("isize::MAX shouldn't trigger an overflow!");
                }

                if guards_against_isize {
                    // Check isize::MAX + 1 does count as overflow
                    if let Err(CapacityOverflow) = empty_bytes.try_reserve(MAX_CAP + 1) {
                    } else { panic!("isize::MAX + 1 should trigger an overflow!") }

                    // Check usize::MAX does count as overflow
                    if let Err(CapacityOverflow) = empty_bytes.try_reserve(MAX_USIZE) {
                    } else { panic!("usize::MAX should trigger an overflow!") }
                } else {
                    // Check isize::MAX + 1 is an OOM
                    if let Err(AllocErr) = empty_bytes.try_reserve(MAX_CAP + 1) {
                    } else { panic!("isize::MAX + 1 should trigger an OOM!") }

                    // Check usize::MAX is an OOM
                    if let Err(AllocErr) = empty_bytes.try_reserve(MAX_USIZE) {
                    } else { panic!("usize::MAX should trigger an OOM!") }
                }
            }


            {
                // Same basic idea, but with non-zero len
                let mut ten_bytes: JackVec<u8> = jack_vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

                if let Err(CapacityOverflow) = ten_bytes.try_reserve(MAX_CAP - 10) {
                    panic!("isize::MAX shouldn't trigger an overflow!");
                }
                if let Err(CapacityOverflow) = ten_bytes.try_reserve(MAX_CAP - 10) {
                    panic!("isize::MAX shouldn't trigger an overflow!");
                }
                if guards_against_isize {
                    if let Err(CapacityOverflow) = ten_bytes.try_reserve(MAX_CAP - 9) {
                    } else { panic!("isize::MAX + 1 should trigger an overflow!"); }
                } else {
                    if let Err(AllocErr) = ten_bytes.try_reserve(MAX_CAP - 9) {
                    } else { panic!("isize::MAX + 1 should trigger an OOM!") }
                }
                // Should always overflow in the add-to-len
                if let Err(CapacityOverflow) = ten_bytes.try_reserve(MAX_USIZE) {
                } else { panic!("usize::MAX should trigger an overflow!") }
            }


            {
                // Same basic idea, but with interesting type size
                let mut ten_u32s: JackVec<u32> = jack_vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

                if let Err(CapacityOverflow) = ten_u32s.try_reserve(MAX_CAP/4 - 10) {
                    panic!("isize::MAX shouldn't trigger an overflow!");
                }
                if let Err(CapacityOverflow) = ten_u32s.try_reserve(MAX_CAP/4 - 10) {
                    panic!("isize::MAX shouldn't trigger an overflow!");
                }
                if guards_against_isize {
                    if let Err(CapacityOverflow) = ten_u32s.try_reserve(MAX_CAP/4 - 9) {
                    } else { panic!("isize::MAX + 1 should trigger an overflow!"); }
                } else {
                    if let Err(AllocErr) = ten_u32s.try_reserve(MAX_CAP/4 - 9) {
                    } else { panic!("isize::MAX + 1 should trigger an OOM!") }
                }
                // Should fail in the mul-by-size
                if let Err(CapacityOverflow) = ten_u32s.try_reserve(MAX_USIZE - 20) {
                } else {
                    panic!("usize::MAX should trigger an overflow!");
                }
            }

        }

        #[test]
        fn test_try_reserve_exact() {

            // This is exactly the same as test_try_reserve with the method changed.
            // See that test for comments.

            const MAX_CAP: usize = isize::MAX as usize;
            const MAX_USIZE: usize = usize::MAX;

            let guards_against_isize = size_of::<usize>() < 8;

            {
                let mut empty_bytes: JackVec<u8> = JackVec::new();

                if let Err(CapacityOverflow) = empty_bytes.try_reserve_exact(MAX_CAP) {
                    panic!("isize::MAX shouldn't trigger an overflow!");
                }
                if let Err(CapacityOverflow) = empty_bytes.try_reserve_exact(MAX_CAP) {
                    panic!("isize::MAX shouldn't trigger an overflow!");
                }

                if guards_against_isize {
                    if let Err(CapacityOverflow) = empty_bytes.try_reserve_exact(MAX_CAP + 1) {
                    } else { panic!("isize::MAX + 1 should trigger an overflow!") }

                    if let Err(CapacityOverflow) = empty_bytes.try_reserve_exact(MAX_USIZE) {
                    } else { panic!("usize::MAX should trigger an overflow!") }
                } else {
                    if let Err(AllocErr) = empty_bytes.try_reserve_exact(MAX_CAP + 1) {
                    } else { panic!("isize::MAX + 1 should trigger an OOM!") }

                    if let Err(AllocErr) = empty_bytes.try_reserve_exact(MAX_USIZE) {
                    } else { panic!("usize::MAX should trigger an OOM!") }
                }
            }


            {
                let mut ten_bytes: JackVec<u8> = jack_vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

                if let Err(CapacityOverflow) = ten_bytes.try_reserve_exact(MAX_CAP - 10) {
                    panic!("isize::MAX shouldn't trigger an overflow!");
                }
                if let Err(CapacityOverflow) = ten_bytes.try_reserve_exact(MAX_CAP - 10) {
                    panic!("isize::MAX shouldn't trigger an overflow!");
                }
                if guards_against_isize {
                    if let Err(CapacityOverflow) = ten_bytes.try_reserve_exact(MAX_CAP - 9) {
                    } else { panic!("isize::MAX + 1 should trigger an overflow!"); }
                } else {
                    if let Err(AllocErr) = ten_bytes.try_reserve_exact(MAX_CAP - 9) {
                    } else { panic!("isize::MAX + 1 should trigger an OOM!") }
                }
                if let Err(CapacityOverflow) = ten_bytes.try_reserve_exact(MAX_USIZE) {
                } else { panic!("usize::MAX should trigger an overflow!") }
            }


            {
                let mut ten_u32s: JackVec<u32> = jack_vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

                if let Err(CapacityOverflow) = ten_u32s.try_reserve_exact(MAX_CAP/4 - 10) {
                    panic!("isize::MAX shouldn't trigger an overflow!");
                }
                if let Err(CapacityOverflow) = ten_u32s.try_reserve_exact(MAX_CAP/4 - 10) {
                    panic!("isize::MAX shouldn't trigger an overflow!");
                }
                if guards_against_isize {
                    if let Err(CapacityOverflow) = ten_u32s.try_reserve_exact(MAX_CAP/4 - 9) {
                    } else { panic!("isize::MAX + 1 should trigger an overflow!"); }
                } else {
                    if let Err(AllocErr) = ten_u32s.try_reserve_exact(MAX_CAP/4 - 9) {
                    } else { panic!("isize::MAX + 1 should trigger an OOM!") }
                }
                if let Err(CapacityOverflow) = ten_u32s.try_reserve_exact(MAX_USIZE - 20) {
                } else { panic!("usize::MAX should trigger an overflow!") }
            }
        }
    */

    #[test]
    fn test_header_data() {
        macro_rules! assert_aligned_head_ptr {
            ($typename:ty) => {{
                let v: JackVec<$typename> = JackVec::with_capacity(1 /* ensure allocation */);
                let head_ptr: *mut $typename = v.data_raw();
                assert_eq!(
                    head_ptr as usize % core::mem::align_of::<$typename>(),
                    0,
                    "expected Header::data<{}> to be aligned",
                    stringify!($typename)
                );
            }};
        }

        const HEADER_SIZE: usize = core::mem::size_of::<Header>();
        assert_eq!(2 * core::mem::size_of::<u32>(), HEADER_SIZE);

        #[repr(C, align(128))]
        struct Funky<T>(T);
        assert_eq!(padding::<Funky<()>>(), 128 - HEADER_SIZE);
        assert_aligned_head_ptr!(Funky<()>);

        assert_eq!(padding::<Funky<u8>>(), 128 - HEADER_SIZE);
        assert_aligned_head_ptr!(Funky<u8>);

        assert_eq!(padding::<Funky<[(); 1024]>>(), 128 - HEADER_SIZE);
        assert_aligned_head_ptr!(Funky<[(); 1024]>);

        assert_eq!(padding::<Funky<[*mut usize; 1024]>>(), 128 - HEADER_SIZE);
        assert_aligned_head_ptr!(Funky<[*mut usize; 1024]>);
    }

    #[cfg(feature = "serde")]
    use serde_test::{assert_tokens, Token};

    #[test]
    #[cfg(feature = "serde")]
    fn test_ser_de_empty() {
        let vec = JackVec::<u32>::new();

        assert_tokens(&vec, &[Token::Seq { len: Some(0) }, Token::SeqEnd]);
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_ser_de() {
        let mut vec = JackVec::<u32>::new();
        vec.push(20);
        vec.push(55);
        vec.push(123);

        assert_tokens(
            &vec,
            &[
                Token::Seq { len: Some(3) },
                Token::U32(20),
                Token::U32(55),
                Token::U32(123),
                Token::SeqEnd,
            ],
        );
    }

    #[test]
    fn test_set_len() {
        let mut vec: JackVec<u32> = jack_vec![];
        unsafe {
            vec.set_len(0); // at one point this caused a crash
        }
    }

    #[test]
    #[should_panic(expected = "invalid set_len(1) on empty JackVec")]
    fn test_set_len_invalid() {
        let mut vec: JackVec<u32> = jack_vec![];
        unsafe {
            vec.set_len(1);
        }
    }

    #[test]
    #[should_panic(expected = "capacity overflow")]
    fn test_capacity_overflow_header_too_big() {
        let vec: JackVec<u8> = JackVec::with_capacity(isize::MAX as usize - 2);
        assert!(vec.capacity() > 0);
    }
    #[test]
    #[should_panic(expected = "capacity overflow")]
    fn test_capacity_overflow_cap_too_big() {
        let vec: JackVec<u8> = JackVec::with_capacity(isize::MAX as usize + 1);
        assert!(vec.capacity() > 0);
    }
    #[test]
    #[should_panic(expected = "capacity overflow")]
    fn test_capacity_overflow_size_mul1() {
        let vec: JackVec<u16> = JackVec::with_capacity(isize::MAX as usize + 1);
        assert!(vec.capacity() > 0);
    }
    #[test]
    #[should_panic(expected = "capacity overflow")]
    fn test_capacity_overflow_size_mul2() {
        let vec: JackVec<u16> = JackVec::with_capacity(isize::MAX as usize / 2 + 1);
        assert!(vec.capacity() > 0);
    }
    #[test]
    #[should_panic(expected = "capacity overflow")]
    fn test_capacity_overflow_cap_really_isnt_isize() {
        let vec: JackVec<u8> = JackVec::with_capacity(isize::MAX as usize);
        assert!(vec.capacity() > 0);
    }

    struct PanicBomb(&'static str);

    impl Drop for PanicBomb {
        fn drop(&mut self) {
            if self.0 == "panic" {
                panic!("panic!");
            }
        }
    }

    #[test]
    #[should_panic(expected = "panic!")]
    fn test_panic_into_iter() {
        let mut v = JackVec::new();
        v.push(PanicBomb("normal1"));
        v.push(PanicBomb("panic"));
        v.push(PanicBomb("normal2"));

        let mut iter = v.into_iter();
        iter.next();
    }

    #[test]
    #[should_panic(expected = "panic!")]
    fn test_panic_clear() {
        let mut v = JackVec::new();
        v.push(PanicBomb("normal1"));
        v.push(PanicBomb("panic"));
        v.push(PanicBomb("normal2"));
        v.clear();
    }
}
