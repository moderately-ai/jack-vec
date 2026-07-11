//! Shared workloads and adapters for the cross-vector comparison suite.

use jack_vec::JackVec;
use smallvec::SmallVec;
use thin_vec::ThinVec;

pub const NESTED_VECTOR_COUNT: usize = 10_000;
pub const GROWING_SIZES: &[usize] = &[1, 4, 1_024];
pub const RESERVED_PUSH_SIZES: &[usize] = &[4, 1_024];
pub const ITERATION_SIZES: &[usize] = &[8, 1_024];
pub const APPEND_SIZES: &[usize] = &[4, 1_024];

pub type SmallVec4<T> = SmallVec<[T; 4]>;
pub type SmallVec8<T> = SmallVec<[T; 8]>;

pub trait BenchVector<T>: Sized {
    const LABEL: &'static str;
    const INLINE_CAPACITY: usize;

    fn new() -> Self;
    fn with_capacity(capacity: usize) -> Self;
    fn push(&mut self, value: T);
    fn append(&mut self, other: &mut Self);
    fn retain_mut<F>(&mut self, predicate: F)
    where
        F: FnMut(&mut T) -> bool;
    fn dedup_by<F>(&mut self, same_bucket: F)
    where
        F: FnMut(&mut T, &mut T) -> bool;
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = T>;
    fn resize(&mut self, new_len: usize, value: T)
    where
        T: Clone;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
    fn capacity(&self) -> usize;
    fn spilled(&self) -> Option<bool>;
    fn as_slice(&self) -> &[T];
}

macro_rules! impl_heap_vector {
    ($type:ident, $label:literal) => {
        impl<T> BenchVector<T> for $type<T> {
            const LABEL: &'static str = $label;
            const INLINE_CAPACITY: usize = 0;

            fn new() -> Self {
                Self::new()
            }
            fn with_capacity(capacity: usize) -> Self {
                Self::with_capacity(capacity)
            }
            fn push(&mut self, value: T) {
                self.push(value);
            }
            fn append(&mut self, other: &mut Self) {
                self.append(other);
            }
            fn retain_mut<F>(&mut self, predicate: F)
            where
                F: FnMut(&mut T) -> bool,
            {
                self.retain_mut(predicate);
            }
            fn dedup_by<F>(&mut self, same_bucket: F)
            where
                F: FnMut(&mut T, &mut T) -> bool,
            {
                self.dedup_by(same_bucket);
            }
            fn extend<I>(&mut self, iter: I)
            where
                I: IntoIterator<Item = T>,
            {
                Extend::extend(self, iter);
            }
            fn resize(&mut self, new_len: usize, value: T)
            where
                T: Clone,
            {
                self.resize(new_len, value);
            }
            fn len(&self) -> usize {
                self.len()
            }
            fn is_empty(&self) -> bool {
                self.is_empty()
            }
            fn capacity(&self) -> usize {
                self.capacity()
            }
            fn spilled(&self) -> Option<bool> {
                None
            }
            fn as_slice(&self) -> &[T] {
                self.as_slice()
            }
        }
    };
}

impl_heap_vector!(Vec, "Vec");
impl_heap_vector!(JackVec, "JackVec");
impl_heap_vector!(ThinVec, "ThinVec");

macro_rules! impl_small_vector {
    ($capacity:literal, $label:literal) => {
        impl<T> BenchVector<T> for SmallVec<[T; $capacity]> {
            const LABEL: &'static str = $label;
            const INLINE_CAPACITY: usize = $capacity;

            fn new() -> Self {
                Self::new()
            }
            fn with_capacity(capacity: usize) -> Self {
                Self::with_capacity(capacity)
            }
            fn push(&mut self, value: T) {
                self.push(value);
            }
            fn append(&mut self, other: &mut Self) {
                self.append(other);
            }
            fn retain_mut<F>(&mut self, predicate: F)
            where
                F: FnMut(&mut T) -> bool,
            {
                self.retain_mut(predicate);
            }
            fn dedup_by<F>(&mut self, same_bucket: F)
            where
                F: FnMut(&mut T, &mut T) -> bool,
            {
                self.dedup_by(same_bucket);
            }
            fn extend<I>(&mut self, iter: I)
            where
                I: IntoIterator<Item = T>,
            {
                Extend::extend(self, iter);
            }
            fn resize(&mut self, new_len: usize, value: T)
            where
                T: Clone,
            {
                self.resize(new_len, value);
            }
            fn len(&self) -> usize {
                self.len()
            }
            fn is_empty(&self) -> bool {
                self.is_empty()
            }
            fn capacity(&self) -> usize {
                self.capacity()
            }
            fn spilled(&self) -> Option<bool> {
                Some(self.spilled())
            }
            fn as_slice(&self) -> &[T] {
                self.as_slice()
            }
        }
    };
}

impl_small_vector!(4, "SmallVec4");
impl_small_vector!(8, "SmallVec8");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NestedWorkload {
    Empty,
    Sparse,
    Small,
}

impl NestedWorkload {
    pub const ALL: [Self; 3] = [Self::Empty, Self::Sparse, Self::Small];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Sparse => "sparse",
            Self::Small => "small",
        }
    }

    pub fn inner_len(self, index: usize) -> usize {
        match self {
            Self::Empty => 0,
            Self::Sparse => match index % 20 {
                0..=15 => 0,
                16..=18 => 1,
                _ => 4,
            },
            Self::Small => 4,
        }
    }
}

pub fn fill_vector<T, V, F>(values: &mut V, len: usize, mut make: F)
where
    V: BenchVector<T>,
    F: FnMut(usize) -> T,
{
    for index in 0..len {
        values.push(make(index));
    }
}

pub fn build_growing<V: BenchVector<u64>>(len: usize) -> V {
    let mut values = V::new();
    fill_vector(&mut values, len, |index| index as u64);
    values
}

pub fn build_reserved<V: BenchVector<u64>>(len: usize) -> V {
    let mut values = V::with_capacity(len);
    fill_vector(&mut values, len, |index| index as u64);
    values
}

pub fn build_nested<V: BenchVector<u64>>(workload: NestedWorkload, count: usize) -> Vec<V> {
    let mut outer = Vec::with_capacity(count);
    for index in 0..count {
        let mut inner = V::new();
        fill_vector(&mut inner, workload.inner_len(index), |offset| {
            (index as u64).wrapping_mul(37).wrapping_add(offset as u64)
        });
        outer.push(inner);
    }
    outer
}

pub fn sum_nested<V: BenchVector<u64>>(values: &[V]) -> u64 {
    values
        .iter()
        .flat_map(|inner| inner.as_slice())
        .fold(0, |sum, value| sum.wrapping_add(*value))
}

pub fn metadata_checksum<V: BenchVector<u64>>(values: &[V]) -> usize {
    values.iter().fold(0, |checksum, inner| {
        checksum
            .wrapping_add(inner.len())
            .wrapping_add(usize::from(inner.is_empty()).rotate_left(31))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exercise<V: BenchVector<u64>>() -> Vec<u64> {
        let mut left = V::with_capacity(4);
        fill_vector(&mut left, 4, |index| index as u64);
        let mut right = V::new();
        fill_vector(&mut right, 4, |index| (index + 4) as u64);
        left.append(&mut right);
        left.retain_mut(|value| *value % 2 == 0);
        left.extend([8, 8, 10]);
        left.dedup_by(|left, right| left == right);
        left.resize(6, 12);
        left.as_slice().to_vec()
    }

    #[test]
    fn adapters_have_identical_semantics() {
        let expected = exercise::<Vec<u64>>();
        assert_eq!(exercise::<JackVec<u64>>(), expected);
        assert_eq!(exercise::<ThinVec<u64>>(), expected);
        assert_eq!(exercise::<SmallVec4<u64>>(), expected);
        assert_eq!(exercise::<SmallVec8<u64>>(), expected);
    }

    #[test]
    fn sparse_distribution_is_exact() {
        let values = build_nested::<Vec<u64>>(NestedWorkload::Sparse, 100);
        assert_eq!(values.iter().filter(|value| value.is_empty()).count(), 80);
        assert_eq!(values.iter().filter(|value| value.len() == 1).count(), 15);
        assert_eq!(values.iter().filter(|value| value.len() == 4).count(), 5);
    }

    #[test]
    fn workload_checksums_match_every_adapter() {
        for workload in NestedWorkload::ALL {
            let expected = build_nested::<Vec<u64>>(workload, 100);
            let expected_sum = sum_nested(&expected);
            let expected_metadata = metadata_checksum(&expected);
            macro_rules! check {
                ($type:ty) => {{
                    let actual = build_nested::<$type>(workload, 100);
                    assert_eq!(sum_nested(&actual), expected_sum);
                    assert_eq!(metadata_checksum(&actual), expected_metadata);
                }};
            }
            check!(JackVec<u64>);
            check!(ThinVec<u64>);
            check!(SmallVec4<u64>);
            check!(SmallVec8<u64>);
        }
    }
}
