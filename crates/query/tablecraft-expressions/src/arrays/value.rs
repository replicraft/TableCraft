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
    /// Adopt an already allocated, charged element buffer without copying it.
    /// An empty array has upper = lower - 1; unrepresentable bounds fail.
    pub fn from_owned(elements: Vec<Value>, lower: i32) -> Result<Self, EvalError> {
        let upper = checked_upper_bound(lower, elements.len())?;

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

fn checked_upper_bound(lower: i32, length: usize) -> Result<i32, EvalError> {
    let length = i64::try_from(length).map_err(|_| EvalError::ArrayBounds)?;
    let upper = i64::from(lower)
        .checked_add(length)
        .and_then(|end| end.checked_sub(1))
        .ok_or(EvalError::ArrayBounds)?;
    i32::try_from(upper).map_err(|_| EvalError::ArrayBounds)
}

#[cfg(test)]
mod tests;

impl fmt::Debug for ArrayValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArrayValue")
            .field("len", &self.elements.len())
            .field("lower", &self.lower)
            .field("upper", &self.upper)
            .finish_non_exhaustive()
    }
}
