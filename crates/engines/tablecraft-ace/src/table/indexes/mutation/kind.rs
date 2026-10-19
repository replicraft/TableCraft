//! Qualified write carriers; physical GUID segments differ from stored UUID bytes.
use super::super::{composite::MixedValue, guid};
use super::IntegralKey;
use crate::{EditError, InspectError};
use tablecraft_schema::ScalarType;

#[derive(Clone, Copy)]
pub(in crate::table) enum MutationKey {
    Integral(IntegralKey),
    Guid,
}
impl MutationKey {
    pub(super) const PLACEHOLDER: Self = Self::Integral(IntegralKey::PLACEHOLDER);
    pub fn new(kind: ScalarType) -> Result<Self, EditError> {
        if kind == ScalarType::Guid {
            Ok(Self::Guid)
        } else {
            IntegralKey::new(kind).map(Self::Integral)
        }
    }
    pub fn stored(self, bytes: &[u8]) -> Result<MixedValue, InspectError> {
        match self {
            Self::Integral(kind) => kind.stored(bytes).map(MixedValue::Numeric),
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
            Self::Integral(kind) => {
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
