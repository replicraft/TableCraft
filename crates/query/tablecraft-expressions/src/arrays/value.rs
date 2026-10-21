use crate::EvalError;
use std::fmt;
use tablecraft_core::values::Value;

/// Immutable one-dimensional Variant-element array with explicit Long bounds.
/// Elements are materialized scalars; nested arrays and language-only elements
/// are outside this profile. Hosts charge allocations before passing ownership.
pub struct ArrayValue {
    elements: Vec<Value>,
    lower: i32,
    upper: i32,
}

impl ArrayValue {
    /// Hard ceiling on Variant-element positions, independent of host budgets.
    pub const MAX_ELEMENTS: usize = 65_536;

    /// Adopt an already allocated, charged element buffer without copying it.
    /// An empty array has upper = lower - 1; unrepresentable bounds fail.
    pub fn from_owned(elements: Vec<Value>, lower: i32) -> Result<Self, EvalError> {
        if elements.len() > Self::MAX_ELEMENTS {
            return Err(EvalError::ArrayLimit);
        }

        let offset = i32::try_from(elements.len()).map_err(|_| EvalError::ArrayBounds)? - 1;
        let upper = lower.checked_add(offset).ok_or(EvalError::ArrayBounds)?;

        Ok(Self {
            elements,
            lower,
            upper,
        })
    }

    /// Borrow elements in their original order, retaining NULLs and zero values.
    pub fn elements(&self) -> &[Value] {
        &self.elements
    }

    /// Smallest index of this array's single dimension.
    pub const fn lower_bound(&self) -> i32 {
        self.lower
    }

    /// Largest index, one below the lower bound for an empty array.
    pub const fn upper_bound(&self) -> i32 {
        self.upper
    }
}

impl fmt::Debug for ArrayValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArrayValue")
            .field("len", &self.elements.len())
            .field("lower", &self.lower)
            .field("upper", &self.upper)
            .finish_non_exhaustive()
    }
}
