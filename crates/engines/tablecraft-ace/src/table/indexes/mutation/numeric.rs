//! Exact native numeric/Boolean/DATE keys; never SQL promotion or calendar rounding.
use super::super::{boolean, floating, profile::KeyType};
use crate::{EditError, InspectError, WriteBlocker};
use tablecraft_core::values::LegacyDateTime;
use tablecraft_schema::ScalarType;

#[derive(Clone, Copy)]
pub(in crate::table) struct NumericKey(KeyType);

impl NumericKey {
    pub(super) const PLACEHOLDER: Self = Self(KeyType::Byte);

    pub fn new(kind: ScalarType) -> Result<Self, EditError> {
        let kind = match kind {
            ScalarType::Boolean => KeyType::Boolean,
            ScalarType::Single => KeyType::Single,
            ScalarType::Double => KeyType::Double,
            ScalarType::DateTime => KeyType::DateTime,
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

    pub(super) fn nullable(self) -> bool {
        self.0 != KeyType::Boolean
    }
    pub(super) fn requires_repack(self) -> bool {
        matches!(
            self.0,
            KeyType::Boolean | KeyType::Single | KeyType::Double | KeyType::DateTime
        )
    }

    pub(super) fn bytes(self) -> usize {
        self.0.bytes()
    }

    #[cfg(any(test, feature = "ace16-write-preview"))]
    pub fn mask_key(self, bit: bool) -> Option<i64> {
        (self.0 == KeyType::Boolean).then(|| boolean::value_key(bit))
    }

    pub fn stored(self, bytes: &[u8]) -> Result<i64, InspectError> {
        Ok(match self.0 {
            KeyType::Boolean => return Err(InspectError::Malformed),
            KeyType::Single => floating::single_key(u32::from_le_bytes(
                bytes.try_into().map_err(|_| InspectError::Malformed)?,
            )),
            KeyType::Double => floating::double_key(u64::from_le_bytes(
                bytes.try_into().map_err(|_| InspectError::Malformed)?,
            )),
            KeyType::DateTime => {
                let bits =
                    u64::from_le_bytes(bytes.try_into().map_err(|_| InspectError::Malformed)?);
                LegacyDateTime::from_bits(bits).map_err(|_| InspectError::Malformed)?;
                floating::double_key(bits)
            }
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
        })
    }

    // Atoms already occupy the reader's signed order domain. Recover the native
    // unsigned bytes without floating arithmetic or a civil DATE round trip.
    // Boolean has its separate two-value encoding; Byte remains unsigned.
    pub(super) fn ordered(self, key: i64) -> Result<[u8; size_of::<i64>()], InspectError> {
        let mut bytes = [0; size_of::<i64>()];
        let end = self.bytes();
        match self.0 {
            KeyType::Boolean => {
                bytes[0] = boolean::encode(key)?;
                return Ok(bytes);
            }
            KeyType::Byte => bytes[0] = u8::try_from(key).map_err(|_| InspectError::Malformed)?,
            KeyType::Int16 => bytes[..end].copy_from_slice(
                &i16::try_from(key)
                    .map_err(|_| InspectError::Malformed)?
                    .to_be_bytes(),
            ),
            KeyType::Int32 | KeyType::Single => bytes[..end].copy_from_slice(
                &i32::try_from(key)
                    .map_err(|_| InspectError::Malformed)?
                    .to_be_bytes(),
            ),
            KeyType::Int64 | KeyType::Currency | KeyType::Double | KeyType::DateTime => {
                bytes[..end].copy_from_slice(&key.to_be_bytes())
            }
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
            let codec = NumericKey::new(kind).unwrap();
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
            assert!(NumericKey::new(kind).unwrap().ordered(key).is_err());
        }
        assert!(NumericKey::new(ScalarType::Guid).is_err());
    }
    #[test]
    fn literal_boolean_ieee_and_date_goldens_preserve_physical_domains() {
        use super::super::super::composite::MixedValue;
        use super::super::MutationKey;
        for (kind, stored, expected) in [
            (
                ScalarType::Single,
                vec![0, 0, 0, 0],
                vec![127, 128, 0, 0, 0],
            ),
            (
                ScalarType::Single,
                vec![0, 0, 0, 128],
                vec![127, 127, 255, 255, 255],
            ),
            (
                ScalarType::Double,
                vec![1, 0, 0, 0, 0, 0, 248, 127],
                vec![127, 255, 248, 0, 0, 0, 0, 0, 1],
            ),
            (
                ScalarType::Double,
                vec![2, 0, 0, 0, 0, 0, 248, 255],
                vec![127, 0, 7, 255, 255, 255, 255, 255, 253],
            ),
            (
                ScalarType::DateTime,
                vec![0, 0, 0, 0, 0, 0, 208, 191],
                vec![127, 64, 47, 255, 255, 255, 255, 255, 255],
            ),
        ] {
            let codec = MutationKey::new(kind).unwrap();
            let key = codec.stored(&stored).unwrap();
            let mut bytes = [0; 9];
            let length = codec.encode(Some(key), false, &mut bytes).unwrap();
            assert_eq!(&bytes[..length], expected);
            assert!(codec.stored(&stored[..stored.len() - 1]).is_err());
        }
        let boolean = MutationKey::new(ScalarType::Boolean).unwrap();
        for (bit, expected) in [(false, [127, 255]), (true, [127, 0])] {
            let atom = boolean.cell(&[], bit).unwrap();
            assert!(matches!(atom, Some(MixedValue::Numeric(_))));
            let mut bytes = [0; 2];
            assert_eq!(boolean.encode(atom, false, &mut bytes).unwrap(), 2);
            assert_eq!(bytes, expected);
        }
        assert!(boolean.encode(None, false, &mut [0; 2]).is_err());
        assert!(
            MutationKey::new(ScalarType::DateTime)
                .unwrap()
                .stored(&f64::INFINITY.to_bits().to_le_bytes())
                .is_err()
        );
    }
}
