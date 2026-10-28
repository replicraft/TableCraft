//! Transport checked original key bytes and explicitly bound row semantics.
#[cfg(test)]
mod tests;

use super::{
    super::{
        key::{KeyProfile, KeyRow, KeyWork},
        page::KeyRef,
    },
    profile::WriteProfile,
    row_key::RowKeyProfile,
};
use crate::{InspectError, header::check};
use std::cmp::Ordering;
use tablecraft_core::limits::Resource;

pub(super) trait RetainedProfile: KeyProfile {
    fn retain(&self, key: &Self::Key, work: &mut KeyWork<'_>) -> Result<Self::Key, InspectError>;
    fn encode_retained(&self, key: &Self::Key, output: &mut [u8]) -> Result<usize, InspectError>;
}

impl<P: WriteProfile> RetainedProfile for P {
    fn retain(&self, key: &Self::Key, _work: &mut KeyWork<'_>) -> Result<Self::Key, InspectError> {
        Ok(*key)
    }

    fn encode_retained(&self, key: &Self::Key, output: &mut [u8]) -> Result<usize, InspectError> {
        self.encode(*key, output)
    }
}

pub(super) fn reference<P: RetainedProfile>(
    profile: &P,
    key: &KeyRef<P::Key>,
    work: &mut KeyWork<'_>,
) -> Result<KeyRef<P::Key>, InspectError> {
    Ok(KeyRef {
        key: profile.retain(&key.key, work)?,
        row: key.row,
    })
}

pub(super) struct Preserved<P>(pub P);

pub(super) struct PreservedKey<K> {
    semantic: K,
    wire: Vec<u8>,
    row_nulls: Option<(bool, bool)>,
}

// Constraint facts do not alter physical key identity or separator comparison.
impl<K: Eq> PartialEq for PreservedKey<K> {
    fn eq(&self, other: &Self) -> bool {
        self.semantic == other.semantic && self.wire == other.wire
    }
}
impl<K: Eq> Eq for PreservedKey<K> {}

impl<P: KeyProfile> KeyProfile for Preserved<P> {
    type Key = PreservedKey<P::Key>;

    fn components(&self) -> usize {
        self.0.components()
    }

    fn max_key_bytes(&self) -> usize {
        self.0.max_key_bytes()
    }

    fn decode(
        &self,
        bytes: &[u8],
        work: &mut KeyWork<'_>,
    ) -> Result<(Self::Key, usize), InspectError> {
        let (semantic, consumed) = self.0.decode(bytes, work)?;
        if consumed != bytes.len() {
            return Err(InspectError::Malformed);
        }
        check(work.budget, work.cancelled)?;
        work.budget.charge_allocation(bytes.len() as u64, 1)?;
        work.budget.charge(Resource::Steps, bytes.len() as u64)?;
        let mut wire = Vec::new();
        wire.try_reserve_exact(bytes.len())
            .map_err(|_| InspectError::Allocation)?;
        wire.extend_from_slice(bytes);
        Ok((
            PreservedKey {
                semantic,
                wire,
                row_nulls: None,
            },
            consumed,
        ))
    }

    fn compare(&self, left: &Self::Key, right: &Self::Key) -> Ordering {
        self.0.compare(&left.semantic, &right.semantic)
    }

    fn constraint_nulls(&self, key: &Self::Key) -> Result<(bool, bool), InspectError> {
        match key.row_nulls {
            Some(nulls) => Ok(nulls),
            None => self.0.constraint_nulls(&key.semantic),
        }
    }

    fn unique_key_is_exact(&self, key: &Self::Key) -> bool {
        self.0.unique_key_is_exact(&key.semantic)
    }

    fn row_key(
        &self,
        row: &(impl KeyRow + ?Sized),
        work: &mut KeyWork<'_>,
    ) -> Result<Self::Key, InspectError> {
        // Comparison-only row keys have no original wire representation. Only
        // decode-produced keys enter deletion serialization; complete checks use
        // the original read profile, including its normal row encoder.
        Ok(PreservedKey {
            semantic: self.0.row_key(row, work)?,
            wire: Vec::new(),
            row_nulls: None,
        })
    }
}

impl<P: KeyProfile> RetainedProfile for Preserved<P> {
    fn retain(&self, key: &Self::Key, work: &mut KeyWork<'_>) -> Result<Self::Key, InspectError> {
        // Re-decode and copy the original bytes under the same paid decoder.
        // This creates no new Text weights or semantic Binary serialization.
        let (mut retained, _) = self.decode(&key.wire, work)?;
        retained.row_nulls = key.row_nulls;
        Ok(retained)
    }

    fn encode_retained(&self, key: &Self::Key, output: &mut [u8]) -> Result<usize, InspectError> {
        if key.wire.is_empty() || key.wire.len() > self.max_key_bytes() {
            return Err(InspectError::Malformed);
        }
        output
            .get_mut(..key.wire.len())
            .ok_or(InspectError::Malformed)?
            .copy_from_slice(&key.wire);
        Ok(key.wire.len())
    }
}

impl<P: RowKeyProfile> Preserved<P> {
    /// Bind a complete INSERT row, retaining NULL facts independently of any
    /// whole-key truncation or digest. Separator decoding never calls this.
    pub(super) fn row_key_with_wire(
        &self,
        row: &(impl KeyRow + ?Sized),
        work: &mut KeyWork<'_>,
    ) -> Result<PreservedKey<P::Key>, InspectError> {
        let semantic = self.0.row_key(row, work)?;
        let row_nulls = self.0.constraint_nulls(&semantic)?;
        let wire = self.0.encoded_row(row, work)?;
        if wire.is_empty() || wire.len() > self.max_key_bytes() {
            return Err(InspectError::Malformed);
        }
        Ok(PreservedKey {
            semantic,
            wire,
            row_nulls: Some(row_nulls),
        })
    }

    /// A source leaf must match the complete immutable row before its NULL facts
    /// become authoritative. Keep the original physical wire byte for byte.
    pub(super) fn bind_row(
        &self,
        original: &PreservedKey<P::Key>,
        row: &(impl KeyRow + ?Sized),
        work: &mut KeyWork<'_>,
    ) -> Result<PreservedKey<P::Key>, InspectError> {
        let bound = self.row_key_with_wire(row, work)?;
        work.budget
            .charge(Resource::Steps, original.wire.len() as u64)?;
        if original.wire != bound.wire || !self.compare(original, &bound).is_eq() {
            return Err(InspectError::Malformed);
        }
        Ok(bound)
    }
}
