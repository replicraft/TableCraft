//! Short keys stay inline; wider fixed tuples own explicitly charged bytes.
use crate::{EditError, InspectError, format::reader::Reader};
use std::{cmp::Ordering, io::Cursor, ops::Deref};
use tablecraft_core::values::Guid;

const MARKER_BYTES: usize = size_of::<u8>();
const TAG_BYTES: usize = size_of::<u8>();
const ROW_BYTES: usize = size_of::<u32>();
const CHILD_BYTES: usize = size_of::<u32>();
const GUID_SEGMENTS: usize = 2;
const INLINE_BYTES: usize = MARKER_BYTES + size_of::<i64>() + ROW_BYTES + CHILD_BYTES;
const MAX_COMPONENTS: usize = 10;
const GUID_KEY_BYTES: usize = MARKER_BYTES + Guid::BYTE_LEN + GUID_SEGMENTS * TAG_BYTES;
pub(super) const MAX_WIRE_BYTES: usize = MAX_COMPONENTS * GUID_KEY_BYTES + ROW_BYTES + CHILD_BYTES;

#[cfg_attr(test, derive(Debug))]
pub(super) enum Wire {
    Inline {
        bytes: [u8; INLINE_BYTES],
        length: usize,
    },
    Owned(Vec<u8>),
}

impl Wire {
    pub(super) fn read(bytes: &[u8]) -> Result<Self, EditError> {
        if bytes.len() > INLINE_BYTES {
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
        Self::read_paid(self, reader)
    }

    pub(super) fn child(&self, number: u32) -> Result<Self, EditError> {
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
        Self::read(&bytes[..end])
    }

    pub(super) fn child_paid(
        &self,
        number: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Self, EditError> {
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
        Self::read_paid(&bytes[..end], reader)
    }
}

impl Deref for Wire {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Inline { bytes, length } => &bytes[..*length],
            Self::Owned(bytes) => bytes,
        }
    }
}

impl PartialEq for Wire {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
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
        self.deref().cmp(other.deref())
    }
}
impl<'a> IntoIterator for &'a Wire {
    type Item = &'a u8;
    type IntoIter = std::slice::Iter<'a, u8>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
