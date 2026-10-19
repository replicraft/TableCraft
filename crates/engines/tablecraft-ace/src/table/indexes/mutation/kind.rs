//! Qualified write carriers; physical GUID segments differ from stored UUID bytes.
use super::super::{composite::DecimalValue, decimal::DecimalType, guid};
use super::NumericKey;
use crate::{EditError, InspectError, NativeColumn};
use tablecraft_schema::ScalarType;

#[derive(Clone, Copy)]
pub(in crate::table) enum MutationKey {
    Numeric(NumericKey),
    Guid,
    Decimal(DecimalType),
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
    pub fn from_column(column: &NativeColumn) -> Result<Self, EditError> {
        if column.scalar_type() == Some(ScalarType::Decimal) {
            Ok(Self::Decimal(DecimalType::from_column(column)?))
        } else {
            Self::new(column.scalar_type().ok_or(InspectError::Unsupported)?)
        }
    }
    // A clear Boolean mask is false, never the NULL atom used by other fields.
    #[cfg(any(test, feature = "ace16-write-preview"))]
    pub fn cell(self, bytes: &[u8], present: bool) -> Result<Option<DecimalValue>, InspectError> {
        if let Self::Numeric(kind) = self
            && let Some(key) = kind.mask_key(present)
        {
            return Ok(Some(DecimalValue::Numeric(key)));
        }
        present.then(|| self.stored(bytes)).transpose()
    }
    pub fn stored(self, bytes: &[u8]) -> Result<DecimalValue, InspectError> {
        match self {
            Self::Numeric(kind) => kind.stored(bytes).map(DecimalValue::Numeric),
            Self::Decimal(kind) => kind.stored_key(bytes).map(DecimalValue::Decimal),
            Self::Guid => Ok(DecimalValue::Guid(
                uuid::Uuid::from_bytes_le(bytes.try_into().map_err(|_| InspectError::Malformed)?)
                    .into_bytes(),
            )),
        }
    }
    pub(super) fn encode(
        self,
        key: Option<DecimalValue>,
        descending: bool,
        bytes: &mut [u8],
    ) -> Result<usize, InspectError> {
        match self {
            Self::Numeric(kind) => {
                super::encoding::component(numeric(key)?, kind, descending, bytes)
            }
            Self::Guid => guid::encode_component(guid_key(key)?, descending, bytes),
            Self::Decimal(kind) => kind.encode(decimal_key(key)?, descending, bytes),
        }
    }
}
pub(super) fn numeric(value: Option<DecimalValue>) -> Result<Option<i64>, InspectError> {
    value
        .map(|value| match value {
            DecimalValue::Numeric(value) => Ok(value),
            _ => Err(InspectError::Malformed),
        })
        .transpose()
}
pub(super) fn guid_key(
    value: Option<DecimalValue>,
) -> Result<Option<[u8; tablecraft_core::values::Guid::BYTE_LEN]>, InspectError> {
    value
        .map(|value| match value {
            DecimalValue::Guid(value) => Ok(value),
            _ => Err(InspectError::Malformed),
        })
        .transpose()
}

#[cfg(test)]
mod tests;

pub(super) fn decimal_key(value: Option<DecimalValue>) -> Result<Option<i128>, InspectError> {
    value
        .map(|v| match v {
            DecimalValue::Decimal(v) => Ok(v),
            _ => Err(InspectError::Malformed),
        })
        .transpose()
}
