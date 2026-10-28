//! Borrowed stored collection values, without query or native-storage dependencies.
use tablecraft_core::values::Value;

/// One parent's stored scalar values, in source order. Empty collections and
/// collections containing NULL remain distinct. Implementors own stable data and
/// callback work limits; get must return Some for every index below len.
/// This view exposes no globally meaningful child identities or writable handles.
pub trait MultivalueList {
    /// Number of stored values, retaining duplicates and NULL cells.
    fn len(&self) -> usize;

    /// Whether there are no stored child values.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Borrow one exact stored scalar value; no coercion or I/O is implied.
    fn get(&self, index: usize) -> Option<&Value>;
}

impl MultivalueList for Vec<Value> {
    fn len(&self) -> usize {
        self.as_slice().len()
    }

    fn get(&self, index: usize) -> Option<&Value> {
        self.as_slice().get(index)
    }
}
