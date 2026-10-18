//! Qualified native index declarations and single-integer consistency rules.
use super::NativeIndex;
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
    /// Whether non-NULL keys must be unique, including a shared primary backing.
    pub const fn unique(self) -> bool {
        self.unique
    }
    /// Whether index keys require non-NULL values. Not a column Required property.
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
        if let Some(key) = key {
            if self.unique && *previous == Some(key) {
                return Err(IndexConstraintViolation::DuplicateKey);
            }
            *previous = Some(key);
        } else if self.required {
            return Err(IndexConstraintViolation::NullKey);
        }
        Ok(())
    }
}
impl NativeIndex {
    /// Decode known local primary/ordinary index declarations. Foreign-reference
    /// types and unknown flags return None; raw metadata remains available.
    pub fn constraints(&self) -> Option<IndexConstraints> {
        if self.kind > PRIMARY_INDEX || self.flags & !KNOWN_FLAGS != 0 {
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
    /// Two records have the same non-NULL integer key in a unique index.
    #[error("duplicate non-NULL key")]
    DuplicateKey,
    /// A primary/required index contains a NULL key.
    #[error("NULL key in primary or required index")]
    NullKey,
}

#[cfg(test)]
mod tests;
