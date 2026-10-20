//! Borrowed scalar collection values, without importing native storage or identity.
use crate::{QueryError, source::ProjectedRow};
use tablecraft_core::values::Value;

/// One parent scalar projection and its requested borrowed collection lists.
pub type MultivalueVisitor<'a> =
    dyn FnMut(ProjectedRow<'_>, &[&dyn MultivalueList]) -> Result<(), QueryError> + 'a;

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
