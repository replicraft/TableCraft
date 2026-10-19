//! Qualified native index declarations and exact numeric-tuple consistency rules.
use super::{NativeIndex, reference::FOREIGN_INDEX};
use thiserror::Error;

pub(super) const PRIMARY_INDEX: u8 = 1;
const UNIQUE: u8 = 0x01;
pub(super) const IGNORE_NULLS: u8 = 0x02;
const REQUIRED: u8 = 0x08;
const ACCESS_2000_MARKER: u8 = 0x80;
const KNOWN_FLAGS: u8 = ACCESS_2000_MARKER | UNIQUE | IGNORE_NULLS | REQUIRED;

/// Decoded index declarations, not field properties or proof of valid records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexConstraints {
    primary: bool,
    unique: bool,
    required: bool,
    ignore_nulls: bool,
}
impl IndexConstraints {
    /// Whether this logical index declares the primary key.
    pub const fn primary_key(self) -> bool {
        self.primary
    }
    /// Whether keys that are not entirely NULL must be unique, including aliases.
    pub const fn unique(self) -> bool {
        self.unique
    }
    /// Whether every key component requires a value. Not a field Required property.
    pub const fn required(self) -> bool {
        self.required
    }
    /// Whether entirely NULL keys are omitted from this physical index.
    pub const fn ignore_nulls(self) -> bool {
        self.ignore_nulls
    }
    pub(super) fn check_key(
        self,
        previous: &mut Option<i64>,
        key: Option<i64>,
    ) -> Result<(), IndexConstraintViolation> {
        self.check_tuple(previous, key, key.is_none())
    }
    pub(super) fn check_tuple<K: Copy + Eq>(
        self,
        previous: &mut Option<K>,
        key: Option<K>,
        any_null: bool,
    ) -> Result<(), IndexConstraintViolation> {
        if any_null && self.required {
            return Err(IndexConstraintViolation::NullKey);
        }
        if let Some(key) = key {
            if self.unique && *previous == Some(key) {
                return Err(IndexConstraintViolation::DuplicateKey);
            }
            *previous = Some(key);
        }
        Ok(())
    }
}
impl NativeIndex {
    /// Decode known local primary/ordinary declarations, or foreign declarations
    /// after explicit reciprocal/catalog validation during an indexed read/check.
    /// Raw inspection never qualifies foreign declarations; unknown flags/types
    /// return None. This does not prove valid records or safe native writes.
    pub fn constraints(&self) -> Option<IndexConstraints> {
        if self.kind > PRIMARY_INDEX && !(self.kind == FOREIGN_INDEX && self.foreign_checked) {
            return None;
        }
        self.physical_constraints()
    }
    // Declaration checks may inspect known flags after validating reciprocal
    // references. This alone never qualifies foreign physical traversal/writes.
    pub(super) fn physical_constraints(&self) -> Option<IndexConstraints> {
        if self.flags & !KNOWN_FLAGS != 0 {
            return None;
        }
        Some(IndexConstraints {
            primary: self.kind == PRIMARY_INDEX,
            unique: self.backing_primary || self.flags & UNIQUE != 0,
            required: self.backing_primary || self.flags & REQUIRED != 0,
            ignore_nulls: self.flags & IGNORE_NULLS != 0,
        })
    }
}

/// A qualified index's decoded rows violate its declared constraint.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum IndexConstraintViolation {
    /// Two records have the same qualified non-NULL key in a unique index.
    #[error("duplicate non-NULL key")]
    DuplicateKey,
    /// A primary/required index contains a NULL key.
    #[error("NULL key in primary or required index")]
    NullKey,
}

#[cfg(test)]
mod tests;
