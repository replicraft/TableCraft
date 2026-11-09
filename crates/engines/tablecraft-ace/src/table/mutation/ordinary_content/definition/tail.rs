//! Row-only retention of opaque user-definition bytes after the declared end.
use crate::{
    EditError, FileFormat, NativeImage,
    format::{
        bytes::u32_at,
        definition::{LENGTH_OFFSET, PAGE_HEADER_BYTES},
        page::PAGE_BYTES,
    },
    header::check,
};
use bytes::Bytes;
use tablecraft_core::limits::{Resource, ResourceBudget};

// Conservative qualification margin for this retained row profile, not a
// claim that every Jet/ACE definition has a native reserved footer.
pub(super) const RESERVED_BYTES: usize = PAGE_HEADER_BYTES;
const CHECK_BYTES: usize = 256;

/// Minted only after complete selected row declaration/index/LVAL framing.
/// This owns immutable original bytes; it exposes no capacity or write access.
pub(in crate::table::mutation) struct RetainedDefinitionTail {
    definition: u32,
    declared_end: usize,
    start: usize,
    original_length: usize,
    bytes: Bytes,
}

impl RetainedDefinitionTail {
    pub(super) fn capture(
        image: &NativeImage,
        definition: u32,
        declared_end: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        check(budget, cancelled)?;
        let base = definition as usize * PAGE_BYTES;
        let start = base
            .checked_add(declared_end)
            .ok_or(EditError::Preservation)?;
        let end = base
            .checked_add(PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        if image.catalog().header().format() != FileFormat::Ace2016
            || declared_end > PAGE_BYTES - RESERVED_BYTES
            || image.as_bytes().get(start..end).is_none()
        {
            return Err(EditError::Preservation);
        }

        // NO_CLONE: Retain an immutable slice of the already paid/shared image;
        // the row Source outlives its borrowed absence certificate. Bytes::slice
        // shares image storage without copying the page payload.
        let bytes = image.bytes.slice(start..end);
        Ok(Self {
            definition,
            declared_end,
            start,
            original_length: image.as_bytes().len(),
            bytes,
        })
    }

    pub(in crate::table::mutation) fn share(&self) -> Self {
        // NO_CLONE: The row Source retains this immutable original after its
        // certificate drops. The Bytes clone shares the paid image allocation;
        // borrowing would force Source to inherit the certificate's lifetime.
        let bytes = self.bytes.clone();
        Self {
            definition: self.definition,
            declared_end: self.declared_end,
            start: self.start,
            original_length: self.original_length,
            bytes,
        }
    }

    pub(in crate::table::mutation) fn require_unchanged(
        &self,
        image: &NativeImage,
        after: &[u8],
        selected_definition: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        check(budget, cancelled)?;
        let end = self
            .start
            .checked_add(self.bytes.len())
            .ok_or(EditError::Preservation)?;
        let original = image
            .as_bytes()
            .get(self.start..end)
            .ok_or(EditError::Preservation)?;
        let current = after.get(self.start..end).ok_or(EditError::Preservation)?;
        let length_at = self.definition as usize * PAGE_BYTES + LENGTH_OFFSET;
        if selected_definition != self.definition
            || image.as_bytes().len() != self.original_length
            || original.as_ptr() != self.bytes.as_ptr()
            || u32_at(image.as_bytes(), length_at)? as usize != self.declared_end
            || u32_at(after, length_at)? as usize != self.declared_end
        {
            return Err(EditError::Preservation);
        }
        for (expected, actual) in self
            .bytes
            .chunks(CHECK_BYTES)
            .zip(current.chunks(CHECK_BYTES))
        {
            check(budget, cancelled)?;
            budget.charge(Resource::Steps, expected.len() as u64)?;
            if expected != actual {
                return Err(EditError::Preservation);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
