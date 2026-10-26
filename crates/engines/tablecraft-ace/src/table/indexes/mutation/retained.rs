//! DELETE transports checked original keys; it never pretends to update them.
#[cfg(test)]
mod tests;

use super::{
    super::{
        key::{KeyProfile, KeyRow, KeyWork},
        page::KeyRef,
    },
    profile::WriteProfile,
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

#[derive(Eq, PartialEq)]
pub(super) struct PreservedKey<K> {
    semantic: K,
    wire: Vec<u8>,
}

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
        Ok((PreservedKey { semantic, wire }, consumed))
    }

    fn compare(&self, left: &Self::Key, right: &Self::Key) -> Ordering {
        self.0.compare(&left.semantic, &right.semantic)
    }

    fn constraint_nulls(&self, key: &Self::Key) -> Result<(bool, bool), InspectError> {
        self.0.constraint_nulls(&key.semantic)
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
        })
    }
}

impl<P: KeyProfile> RetainedProfile for Preserved<P> {
    fn retain(&self, key: &Self::Key, work: &mut KeyWork<'_>) -> Result<Self::Key, InspectError> {
        // Re-decode and copy the original bytes under the same paid decoder.
        // This creates no new Text weights or semantic Binary serialization.
        self.decode(&key.wire, work).map(|(key, _)| key)
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
