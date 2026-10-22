//! Fallible raw string-byte output with shared payload/work accounting.
use crate::{EvalError, functions::text::output::ByteOutput};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) struct Output<'a> {
    pub budget: &'a mut ResourceBudget,
    pub cancelled: Option<&'a dyn Fn() -> bool>,
    pub max_bytes: usize,
    pub bytes: Vec<u8>,
}

impl ByteOutput for Output<'_> {
    fn step(&mut self) -> Result<(), EvalError> {
        if self.cancelled.is_some_and(|cancelled| cancelled()) {
            return Err(EvalError::Cancelled);
        }
        self.budget.charge(Resource::Steps, 1)?;
        Ok(())
    }

    fn append(&mut self, fragment: &[u8]) -> Result<(), EvalError> {
        self.step()?;
        let length = self
            .bytes
            .len()
            .checked_add(fragment.len())
            .ok_or(EvalError::TextLimit)?;
        if length > self.max_bytes {
            return Err(EvalError::TextLimit);
        }
        self.budget.charge(Resource::Bytes, fragment.len() as u64)?;
        self.bytes
            .try_reserve(fragment.len())
            .map_err(|_| EvalError::Allocation)?;
        self.bytes.extend_from_slice(fragment);
        Ok(())
    }
}
