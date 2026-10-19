//! Exact integer and scaled-Currency keys; no numeric promotion or rounding.
use super::super::profile::KeyType;
use crate::{EditError, InspectError, WriteBlocker};
use tablecraft_schema::ScalarType;

#[derive(Clone, Copy)]
pub(in crate::table) struct IntegralKey(KeyType);

impl IntegralKey {
    pub fn new(kind: ScalarType) -> Result<Self, EditError> {
        let kind = match kind {
            ScalarType::Byte => KeyType::Byte,
            ScalarType::Int16 => KeyType::Int16,
            ScalarType::Int32 => KeyType::Int32,
            ScalarType::Int64 => KeyType::Int64,
            ScalarType::Currency => KeyType::Currency,
            _ => return Err(EditError::Unsupported(WriteBlocker::IndexedColumn)),
        };
        Ok(Self(kind))
    }

    pub(super) fn key_type(self) -> KeyType {
        self.0
    }

    pub(super) fn bytes(self) -> usize {
        self.0.bytes()
    }

    pub fn stored(self, bytes: &[u8]) -> Result<i64, InspectError> {
        Ok(match self.0 {
            KeyType::Byte => i64::from(u8::from_le_bytes(
                bytes.try_into().map_err(|_| InspectError::Malformed)?,
            )),
            KeyType::Int16 => i64::from(i16::from_le_bytes(
                bytes.try_into().map_err(|_| InspectError::Malformed)?,
            )),
            KeyType::Int32 => i64::from(i32::from_le_bytes(
                bytes.try_into().map_err(|_| InspectError::Malformed)?,
            )),
            KeyType::Int64 | KeyType::Currency => {
                i64::from_le_bytes(bytes.try_into().map_err(|_| InspectError::Malformed)?)
            }
            _ => return Err(InspectError::Unsupported),
        })
    }

    // Reuse the exact signed domain, then retain only the native width. Byte is
    // unsigned; the signed carriers flip their sign bit before big-endian output.
    pub(super) fn ordered(self, key: i64) -> Result<[u8; size_of::<i64>()], InspectError> {
        let mut bytes = [0; size_of::<i64>()];
        let end = self.bytes();
        match self.0 {
            KeyType::Byte => bytes[0] = u8::try_from(key).map_err(|_| InspectError::Malformed)?,
            KeyType::Int16 => bytes[..end].copy_from_slice(
                &i16::try_from(key)
                    .map_err(|_| InspectError::Malformed)?
                    .to_be_bytes(),
            ),
            KeyType::Int32 => bytes[..end].copy_from_slice(
                &i32::try_from(key)
                    .map_err(|_| InspectError::Malformed)?
                    .to_be_bytes(),
            ),
            KeyType::Int64 | KeyType::Currency => bytes[..end].copy_from_slice(&key.to_be_bytes()),
            _ => return Err(InspectError::Unsupported),
        }
        if self.0 != KeyType::Byte {
            bytes[0] ^= 1 << (u8::BITS - 1);
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_integer_ordering_goldens_preserve_width_and_currency_units() {
        for (kind, key, stored, ordered) in [
            (ScalarType::Byte, 255, vec![255], vec![255]),
            (ScalarType::Int16, -32768, vec![0, 128], vec![0, 0]),
            (ScalarType::Int16, 32767, vec![255, 127], vec![255, 255]),
            (
                ScalarType::Int32,
                -1,
                vec![255; 4],
                vec![127, 255, 255, 255],
            ),
            (
                ScalarType::Int64,
                i64::MIN,
                vec![0, 0, 0, 0, 0, 0, 0, 128],
                vec![0; 8],
            ),
            (
                ScalarType::Int64,
                i64::MAX,
                vec![255, 255, 255, 255, 255, 255, 255, 127],
                vec![255; 8],
            ),
            (
                ScalarType::Currency,
                -1,
                vec![255; 8],
                vec![127, 255, 255, 255, 255, 255, 255, 255],
            ),
            (
                ScalarType::Currency,
                1,
                vec![1, 0, 0, 0, 0, 0, 0, 0],
                vec![128, 0, 0, 0, 0, 0, 0, 1],
            ),
        ] {
            let codec = IntegralKey::new(kind).unwrap();
            assert_eq!(codec.stored(&stored).unwrap(), key);
            assert_eq!(&codec.ordered(key).unwrap()[..codec.bytes()], ordered);
            assert!(codec.stored(&stored[..stored.len() - 1]).is_err());
        }
        for (kind, key) in [
            (ScalarType::Byte, -1),
            (ScalarType::Byte, 256),
            (ScalarType::Int16, 32768),
            (ScalarType::Int32, i64::MAX),
        ] {
            assert!(IntegralKey::new(kind).unwrap().ordered(key).is_err());
        }
        for kind in [
            ScalarType::Boolean,
            ScalarType::Single,
            ScalarType::Double,
            ScalarType::DateTime,
            ScalarType::Guid,
        ] {
            assert!(IntegralKey::new(kind).is_err());
        }
    }
}
