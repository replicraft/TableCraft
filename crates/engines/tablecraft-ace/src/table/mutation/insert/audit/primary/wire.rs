//! Short keys stay inline; wider fixed tuples own explicitly charged bytes.
use crate::{EditError, InspectError, format::reader::Reader};
use std::{cmp::Ordering, io::Cursor, ops::Deref};

const MARKER_BYTES: usize = size_of::<u8>();
const TAG_BYTES: usize = size_of::<u8>();
const ROW_BYTES: usize = size_of::<u32>();
const CHILD_BYTES: usize = size_of::<u32>();
const INLINE_BYTES: usize = MARKER_BYTES + size_of::<i64>() + ROW_BYTES + CHILD_BYTES;
const MAX_SHORT_BINARY: usize = 510;
const BINARY_SEGMENT_BYTES: usize = 8;
const MAX_BINARY_KEY_BYTES: usize = MARKER_BYTES
    + MAX_SHORT_BINARY.div_ceil(BINARY_SEGMENT_BYTES) * (BINARY_SEGMENT_BYTES + TAG_BYTES);
pub(super) const MAX_WIRE_BYTES: usize = MAX_BINARY_KEY_BYTES + ROW_BYTES + CHILD_BYTES;

#[cfg_attr(test, derive(Debug))]
pub(super) enum Wire {
    Inline {
        bytes: [u8; INLINE_BYTES],
        length: usize,
    },
    Owned(Vec<u8>),
    BranchInline {
        bytes: [u8; INLINE_BYTES],
        length: usize,
    },
    BranchOwned(Vec<u8>),
}

impl Wire {
    pub(super) fn read(bytes: &[u8]) -> Result<Self, EditError> {
        if !(MARKER_BYTES + ROW_BYTES..=INLINE_BYTES).contains(&bytes.len()) {
            return Err(EditError::Preservation);
        }
        let mut encoded = [0; INLINE_BYTES];
        encoded[..bytes.len()].copy_from_slice(bytes);
        Ok(Self::Inline {
            bytes: encoded,
            length: bytes.len(),
        })
    }

    pub(super) fn read_paid(
        bytes: &[u8],
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Self, EditError> {
        if bytes.len() <= INLINE_BYTES {
            return Self::read(bytes);
        }
        if bytes.len() > MAX_WIRE_BYTES {
            return Err(EditError::Preservation);
        }
        reader
            .budget
            .charge_allocation(bytes.len() as u64, 1)
            .map_err(InspectError::from)?;
        reader.work(bytes.len())?;
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(bytes.len())
            .map_err(|_| InspectError::Allocation)?;
        owned.extend_from_slice(bytes);
        Ok(Self::Owned(owned))
    }

    pub(super) fn copy_paid(
        &self,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Self, EditError> {
        if self.is_branch() {
            Self::read_branch_paid(self, reader)
        } else {
            Self::read_paid(self, reader)
        }
    }

    pub(super) fn read_branch_paid(
        bytes: &[u8],
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Self, EditError> {
        if bytes.len() < MARKER_BYTES + ROW_BYTES + CHILD_BYTES {
            return Err(EditError::Preservation);
        }
        Ok(Self::read_paid(bytes, reader)?.into_branch())
    }

    fn into_branch(self) -> Self {
        match self {
            Self::Inline { bytes, length } => Self::BranchInline { bytes, length },
            Self::Owned(bytes) => Self::BranchOwned(bytes),
            branch => branch,
        }
    }

    fn is_branch(&self) -> bool {
        matches!(self, Self::BranchInline { .. } | Self::BranchOwned(_))
    }

    fn comparison_parts(&self) -> (&[u8], &[u8], &[u8]) {
        let end = self.len() - if self.is_branch() { CHILD_BYTES } else { 0 };
        let key_end = end - ROW_BYTES;
        (&self[..key_end], &self[key_end..end], &self[end..])
    }

    pub(super) fn child(&self, number: u32) -> Result<Self, EditError> {
        if self.is_branch() || self.len() < MARKER_BYTES + ROW_BYTES {
            return Err(EditError::Preservation);
        }
        let end = self
            .len()
            .checked_add(CHILD_BYTES)
            .ok_or(EditError::Preservation)?;
        if end > INLINE_BYTES {
            return Err(EditError::Preservation);
        }
        let mut bytes = [0; INLINE_BYTES];
        bytes[..self.len()].copy_from_slice(self);
        bytes[self.len()..end].copy_from_slice(&number.to_be_bytes());
        Ok(Self::read(&bytes[..end])?.into_branch())
    }

    pub(super) fn child_paid(
        &self,
        number: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Self, EditError> {
        if self.is_branch() || self.len() < MARKER_BYTES + ROW_BYTES {
            return Err(EditError::Preservation);
        }
        if self.len() + CHILD_BYTES <= INLINE_BYTES {
            return self.child(number);
        }
        let end = self
            .len()
            .checked_add(CHILD_BYTES)
            .ok_or(EditError::Preservation)?;
        if end > MAX_WIRE_BYTES {
            return Err(EditError::Preservation);
        }
        let mut bytes = [0; MAX_WIRE_BYTES];
        bytes[..self.len()].copy_from_slice(self);
        bytes[self.len()..end].copy_from_slice(&number.to_be_bytes());
        Self::read_branch_paid(&bytes[..end], reader)
    }
}

/// Prepay comparisons by actual encoded width, retaining the short-key floor.
pub(super) fn sort_paid(
    entries: &mut [Wire],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let mut width = size_of::<Wire>();
    for entry in entries.iter() {
        reader.tick()?;
        width = width.max(entry.len());
    }

    let levels = entries.len().checked_ilog2().unwrap_or(0) as usize + 1;
    let work = entries
        .len()
        .checked_mul(levels)
        .and_then(|work| work.checked_mul(width))
        .ok_or(InspectError::StructureLimit)?;
    reader.work(work)?;
    entries.sort_unstable();

    Ok(())
}

#[cfg(test)]
mod tests;

impl Deref for Wire {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Inline { bytes, length } | Self::BranchInline { bytes, length } => {
                &bytes[..*length]
            }
            Self::Owned(bytes) | Self::BranchOwned(bytes) => bytes,
        }
    }
}

impl PartialEq for Wire {
    fn eq(&self, other: &Self) -> bool {
        self.is_branch() == other.is_branch() && **self == **other
    }
}
impl Eq for Wire {}
impl PartialOrd for Wire {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Wire {
    fn cmp(&self, other: &Self) -> Ordering {
        // Native entry order compares the complete key first, then its ROW.
        // ROW bytes cannot participate in a variable-length key comparison.
        let (key, row, child) = self.comparison_parts();
        let (other_key, other_row, other_child) = other.comparison_parts();
        key.cmp(other_key)
            .then_with(|| row.cmp(other_row))
            // Child/kind ties keep Ord consistent with physical-byte equality.
            // They never authorize duplicate key/ROW bounds: source bounds are
            // stripped to leaf form before their strict-order qualification.
            .then_with(|| child.cmp(other_child))
            .then_with(|| self.is_branch().cmp(&other.is_branch()))
    }
}
impl<'a> IntoIterator for &'a Wire {
    type Item = &'a u8;
    type IntoIter = std::slice::Iter<'a, u8>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
