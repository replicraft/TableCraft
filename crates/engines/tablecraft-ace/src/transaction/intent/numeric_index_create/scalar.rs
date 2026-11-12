//! Closed native scalar tags; no Rust enum representation crosses the wire.
use super::NumericIndexCreateIntentError as Error;
use crate::format::data_type;
use tablecraft_schema::ScalarType;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum NumericType {
    Boolean,
    Byte,
    Int16,
    Int32,
    Int64,
    Currency,
    Single,
    Double,
    DateTime,
}

impl NumericType {
    pub(super) fn from_native(tag: u8) -> Result<Self, Error> {
        match tag {
            data_type::BOOLEAN => Ok(Self::Boolean),
            data_type::BYTE => Ok(Self::Byte),
            data_type::INT16 => Ok(Self::Int16),
            data_type::INT32 => Ok(Self::Int32),
            data_type::INT64 => Ok(Self::Int64),
            data_type::CURRENCY => Ok(Self::Currency),
            data_type::SINGLE => Ok(Self::Single),
            data_type::DOUBLE => Ok(Self::Double),
            data_type::DATE_TIME => Ok(Self::DateTime),
            _ => Err(Error::InvalidRecord),
        }
    }

    #[cfg(feature = "ace16-create")]
    pub(super) fn from_scalar(scalar: ScalarType) -> Result<Self, Error> {
        match scalar {
            ScalarType::Boolean => Ok(Self::Boolean),
            ScalarType::Byte => Ok(Self::Byte),
            ScalarType::Int16 => Ok(Self::Int16),
            ScalarType::Int32 => Ok(Self::Int32),
            ScalarType::Int64 => Ok(Self::Int64),
            ScalarType::Currency => Ok(Self::Currency),
            ScalarType::Single => Ok(Self::Single),
            ScalarType::Double => Ok(Self::Double),
            ScalarType::DateTime => Ok(Self::DateTime),
            _ => Err(Error::InvalidRecord),
        }
    }

    pub(super) const fn native_type(self) -> u8 {
        match self {
            Self::Boolean => data_type::BOOLEAN,
            Self::Byte => data_type::BYTE,
            Self::Int16 => data_type::INT16,
            Self::Int32 => data_type::INT32,
            Self::Int64 => data_type::INT64,
            Self::Currency => data_type::CURRENCY,
            Self::Single => data_type::SINGLE,
            Self::Double => data_type::DOUBLE,
            Self::DateTime => data_type::DATE_TIME,
        }
    }

    pub(super) const fn scalar_type(self) -> ScalarType {
        match self {
            Self::Boolean => ScalarType::Boolean,
            Self::Byte => ScalarType::Byte,
            Self::Int16 => ScalarType::Int16,
            Self::Int32 => ScalarType::Int32,
            Self::Int64 => ScalarType::Int64,
            Self::Currency => ScalarType::Currency,
            Self::Single => ScalarType::Single,
            Self::Double => ScalarType::Double,
            Self::DateTime => ScalarType::DateTime,
        }
    }
}
