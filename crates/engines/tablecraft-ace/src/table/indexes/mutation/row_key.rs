//! Paid owned row-key encoding shared by native mutation consumers.
use super::super::{
    binary, encoded_tuple,
    key::{KeyProfile, KeyRow, KeyWork},
    text,
};
use super::profile::WriteProfile;
use crate::{InspectError, header::check};
use tablecraft_core::limits::Resource;

pub(super) trait RowKeyProfile: KeyProfile {
    fn encoded_row(
        &self,
        row: &(impl KeyRow + ?Sized),
        work: &mut KeyWork<'_>,
    ) -> Result<Vec<u8>, InspectError>;
}

impl<P: WriteProfile> RowKeyProfile for P {
    fn encoded_row(
        &self,
        row: &(impl KeyRow + ?Sized),
        work: &mut KeyWork<'_>,
    ) -> Result<Vec<u8>, InspectError> {
        let key = self.row_key(row, work)?;
        check(work.budget, work.cancelled)?;
        let capacity = self.max_key_bytes();
        work.budget.charge_allocation(capacity as u64, 1)?;
        work.budget.charge(Resource::Steps, capacity as u64)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(capacity)
            .map_err(|_| InspectError::Allocation)?;
        bytes.resize(capacity, 0);
        let length = self.encode(key, &mut bytes)?;
        if length == 0 || length > capacity {
            return Err(InspectError::Malformed);
        }
        bytes.truncate(length);
        Ok(bytes)
    }
}

impl RowKeyProfile for text::TextProfile {
    fn encoded_row(
        &self,
        row: &(impl KeyRow + ?Sized),
        work: &mut KeyWork<'_>,
    ) -> Result<Vec<u8>, InspectError> {
        self.row_key(row, work).map(|key| key.into_bytes())
    }
}

impl RowKeyProfile for encoded_tuple::EncodedTupleProfile<'_> {
    fn encoded_row(
        &self,
        row: &(impl KeyRow + ?Sized),
        work: &mut KeyWork<'_>,
    ) -> Result<Vec<u8>, InspectError> {
        self.row_key(row, work).map(|key| key.into_bytes())
    }
}

impl RowKeyProfile for binary::BinaryProfile<'_> {
    fn encoded_row(
        &self,
        row: &(impl KeyRow + ?Sized),
        work: &mut KeyWork<'_>,
    ) -> Result<Vec<u8>, InspectError> {
        let mut encoded = [0; binary::MAX_ENTRY_BYTES];
        let length = binary::encode_component(
            row.get(self.column).ok_or(InspectError::Malformed)?,
            self.maximum,
            self.descending,
            &mut encoded,
        )?;
        check(work.budget, work.cancelled)?;
        work.budget.charge_allocation(length as u64, 1)?;
        work.budget.charge(Resource::Steps, length as u64)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| InspectError::Allocation)?;
        bytes.extend_from_slice(&encoded[..length]);
        Ok(bytes)
    }
}
