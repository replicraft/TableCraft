//! Bounded audit-owned encoded entries with their actual physical length.
use crate::EditError;
use std::{cmp::Ordering, ops::Deref};

pub(super) const MAX_WIRE_BYTES: usize = size_of::<u8>() + size_of::<i64>() + 2 * size_of::<u32>();

#[derive(Clone, Copy)]
#[cfg_attr(test, derive(Debug))]
pub(super) struct Wire {
    bytes: [u8; MAX_WIRE_BYTES],
    length: usize,
}

impl Wire {
    pub(super) fn read(bytes: &[u8]) -> Result<Self, EditError> {
        if bytes.len() > MAX_WIRE_BYTES {
            return Err(EditError::Preservation);
        }
        let mut result = Self {
            bytes: [0; MAX_WIRE_BYTES],
            length: bytes.len(),
        };
        result.bytes[..bytes.len()].copy_from_slice(bytes);
        Ok(result)
    }

    pub(super) fn child(self, number: u32) -> Result<Self, EditError> {
        let end = self
            .length
            .checked_add(size_of::<u32>())
            .ok_or(EditError::Preservation)?;
        if end > MAX_WIRE_BYTES {
            return Err(EditError::Preservation);
        }
        let mut result = self;
        result.bytes[self.length..end].copy_from_slice(&number.to_be_bytes());
        result.length = end;
        Ok(result)
    }
}

impl Deref for Wire {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        &self.bytes[..self.length]
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
