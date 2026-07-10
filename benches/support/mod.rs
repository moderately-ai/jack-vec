use jackvec::JackVec;

pub const NESTED_VECTOR_COUNT: usize = 10_000;
pub const OPERATION_SIZES: &[usize] = &[1, 4, 1_024];
pub const ITERATION_SIZES: &[usize] = &[8, 1_024];
pub const APPEND_SIZES: &[usize] = &[4, 1_024];

pub trait BenchVector<T>: Sized {
    const LABEL: &'static str;

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
    fn as_slice(&self) -> &[T];
}

impl<T> BenchVector<T> for Vec<T> {
    const LABEL: &'static str = "Vec";

    fn new() -> Self {
        Self::new()
    }

    fn with_capacity(capacity: usize) -> Self {
        Self::with_capacity(capacity)
    }

    fn push(&mut self, value: T) {
        Vec::push(self, value);
    }

    fn append(&mut self, other: &mut Self) {
        Vec::append(self, other);
    }

    fn retain_mut<F>(&mut self, predicate: F)
    where
        F: FnMut(&mut T) -> bool,
    {
        Vec::retain_mut(self, predicate);
    }

    fn dedup_by<F>(&mut self, same_bucket: F)
    where
        F: FnMut(&mut T, &mut T) -> bool,
    {
        Vec::dedup_by(self, same_bucket);
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
        Vec::resize(self, new_len, value);
    }

    fn as_slice(&self) -> &[T] {
        Vec::as_slice(self)
    }
}

impl<T> BenchVector<T> for JackVec<T> {
    const LABEL: &'static str = "JackVec";

    fn new() -> Self {
        Self::new()
    }

    fn with_capacity(capacity: usize) -> Self {
        Self::with_capacity(capacity)
    }

    fn push(&mut self, value: T) {
        JackVec::push(self, value);
    }

    fn append(&mut self, other: &mut Self) {
        JackVec::append(self, other);
    }

    fn retain_mut<F>(&mut self, predicate: F)
    where
        F: FnMut(&mut T) -> bool,
    {
        JackVec::retain_mut(self, predicate);
    }

    fn dedup_by<F>(&mut self, same_bucket: F)
    where
        F: FnMut(&mut T, &mut T) -> bool,
    {
        JackVec::dedup_by(self, same_bucket);
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
        JackVec::resize(self, new_len, value);
    }

    fn as_slice(&self) -> &[T] {
        JackVec::as_slice(self)
    }
}

#[derive(Clone, Copy)]
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

    fn inner_len(self, index: usize) -> usize {
        match self {
            Self::Empty => 0,
            // 80% empty, 15% one element, and 5% four elements.
            Self::Sparse => match index % 20 {
                0..=15 => 0,
                16..=18 => 1,
                _ => 4,
            },
            Self::Small => 4,
        }
    }
}

pub fn build_growing<V: BenchVector<u64>>(len: usize) -> V {
    let mut values = V::new();
    fill_vector(&mut values, len, 0);
    values
}

pub fn build_reserved<V: BenchVector<u64>>(len: usize) -> V {
    let mut values = V::with_capacity(len);
    fill_vector(&mut values, len, 0);
    values
}

pub fn build_nested<V: BenchVector<u64>>(workload: NestedWorkload, count: usize) -> Vec<V> {
    let mut outer = Vec::with_capacity(count);
    for index in 0..count {
        let len = workload.inner_len(index);
        let mut inner = V::new();
        fill_vector(&mut inner, len, index as u64);
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

pub fn sum_vector<V: BenchVector<u64>>(values: &V) -> u64 {
    values
        .as_slice()
        .iter()
        .fold(0, |sum, value| sum.wrapping_add(*value))
}

pub fn fill_vector<V: BenchVector<u64>>(values: &mut V, len: usize, seed: u64) {
    for index in 0..len {
        values.push(seed.wrapping_mul(37).wrapping_add(index as u64));
    }
}
