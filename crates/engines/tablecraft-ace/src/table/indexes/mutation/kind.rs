//! Qualified write carriers; physical GUID segments differ from stored UUID bytes.
use super::super::{composite::MixedValue, guid};
use super::NumericKey;
use crate::{EditError, InspectError};
use tablecraft_schema::ScalarType;

#[derive(Clone, Copy)]
pub(in crate::table) enum MutationKey {
    Numeric(NumericKey),
    Guid,
}
impl MutationKey {
    pub(super) const PLACEHOLDER: Self = Self::Numeric(NumericKey::PLACEHOLDER);
    pub fn new(kind: ScalarType) -> Result<Self, EditError> {
        if kind == ScalarType::Guid {
            Ok(Self::Guid)
        } else {
            NumericKey::new(kind).map(Self::Numeric)
        }
    }
    // A clear Boolean mask is false, never the NULL atom used by other fields.
    #[cfg(any(test, feature = "ace16-write-preview"))]
    pub fn cell(self, bytes: &[u8], present: bool) -> Result<Option<MixedValue>, InspectError> {
        if let Self::Numeric(kind) = self
            && let Some(key) = kind.mask_key(present)
        {
            return Ok(Some(MixedValue::Numeric(key)));
        }
        present.then(|| self.stored(bytes)).transpose()
    }
    pub fn stored(self, bytes: &[u8]) -> Result<MixedValue, InspectError> {
        match self {
            Self::Numeric(kind) => kind.stored(bytes).map(MixedValue::Numeric),
            Self::Guid => Ok(MixedValue::Guid(
                uuid::Uuid::from_bytes_le(bytes.try_into().map_err(|_| InspectError::Malformed)?)
                    .into_bytes(),
            )),
        }
    }
    pub(super) fn encode(
        self,
        key: Option<MixedValue>,
        descending: bool,
        bytes: &mut [u8],
    ) -> Result<usize, InspectError> {
        match self {
            Self::Numeric(kind) => {
                super::encoding::component(numeric(key)?, kind, descending, bytes)
            }
            Self::Guid => guid::encode_component(guid_key(key)?, descending, bytes),
        }
    }
}
pub(super) fn numeric(value: Option<MixedValue>) -> Result<Option<i64>, InspectError> {
    value
        .map(|value| match value {
            MixedValue::Numeric(value) => Ok(value),
            _ => Err(InspectError::Malformed),
        })
        .transpose()
}
pub(super) fn guid_key(
    value: Option<MixedValue>,
) -> Result<Option<[u8; tablecraft_core::values::Guid::BYTE_LEN]>, InspectError> {
    value
        .map(|value| match value {
            MixedValue::Guid(value) => Ok(value),
            _ => Err(InspectError::Malformed),
        })
        .transpose()
}

#[cfg(test)]
mod tests;
