//! Physical fixed-point index keys; exact field scale stays separate from SQL coercion.
use super::{
    key::{ASC_NULL, ASC_VALUE, DESC_NULL, DESC_VALUE, KeyProfile, MARKER_BYTES},
    page::{CHILD_BYTES, ROW_BYTES},
};
use crate::table::indexes::key::KeyRow;
use crate::table::indexes::key::KeyWork;
use crate::{InspectError, NativeColumn, ReadProfile, format::data_type::NUMERIC_BYTES};
use std::cmp::Ordering;
#[cfg(test)]
use tablecraft_core::values::Decimal;
use tablecraft_core::values::Value;
use tablecraft_schema::{DecimalConstraints, ScalarType};

const POSITIVE: u8 = 0xff;
const NEGATIVE: u8 = 0x00;
const SIGN_BYTES: usize = size_of::<u8>();
// Native Decimal precision is at most 28. This disjoint band preserves the
// legacy descending sign-group order without changing stored/SQL coefficients.
const LEGACY_NEGATIVE_BAND: i128 = 2 * 10_i128.pow(DecimalConstraints::MAX_PRECISION as u32);
pub(super) const KEY_BYTES: usize = MARKER_BYTES + NUMERIC_BYTES;
pub(super) const MAX_ENTRY_BYTES: usize = KEY_BYTES + ROW_BYTES + CHILD_BYTES;

#[derive(Clone, Copy)]
pub(in crate::table) struct DecimalType {
    precision: u8,
    scale: u8,
    legacy: bool,
}
impl DecimalType {
    pub fn from_column(column: &NativeColumn) -> Result<Self, InspectError> {
        if !matches!(
            column.read_profile(),
            ReadProfile::Jet4 | ReadProfile::Ace2007 | ReadProfile::Ace2016 | ReadProfile::Ace2019
        ) || column.scalar_type() != Some(ScalarType::Decimal)
        {
            return Err(InspectError::Unsupported);
        }
        let (precision, scale) = column.decimal_parameters().ok_or(InspectError::Malformed)?;
        Ok(Self {
            precision,
            scale,
            legacy: column.read_profile() == ReadProfile::Jet4,
        })
    }
    pub fn stored_key(self, bytes: &[u8]) -> Result<i128, InspectError> {
        Ok(crate::table::decimal::decode(bytes, self.precision, self.scale)?.coefficient())
    }
    pub fn encode(
        self,
        key: Option<i128>,
        descending: bool,
        bytes: &mut [u8],
    ) -> Result<usize, InspectError> {
        if self.legacy {
            return Err(InspectError::Unsupported);
        }
        self.encode_native(key, descending, bytes)
    }
    /// Read validation reconstructs legacy sign bytes too; write qualification
    /// deliberately remains restricted by encode above.
    pub(super) fn encode_read_component(
        self,
        value: &Value,
        descending: bool,
        bytes: &mut [u8],
    ) -> Result<usize, InspectError> {
        self.value_key(value, descending)?;
        let coefficient = match value {
            Value::Null => None,
            Value::Decimal(value) => Some(value.coefficient()),
            _ => return Err(InspectError::Malformed),
        };
        self.encode_native(coefficient, descending, bytes)
    }
    fn encode_native(
        self,
        key: Option<i128>,
        descending: bool,
        bytes: &mut [u8],
    ) -> Result<usize, InspectError> {
        *bytes.first_mut().ok_or(InspectError::Malformed)? = match (key, descending) {
            (None, false) => ASC_NULL,
            (None, true) => DESC_NULL,
            (Some(_), false) => ASC_VALUE,
            (Some(_), true) => DESC_VALUE,
        };
        let Some(key) = key else {
            return Ok(MARKER_BYTES);
        };
        if key.unsigned_abs() >= self.limit() {
            return Err(InspectError::Malformed);
        }
        let raw = bytes
            .get_mut(MARKER_BYTES..KEY_BYTES)
            .ok_or(InspectError::Malformed)?;
        let negative = key < 0;
        raw[0] = if negative { NEGATIVE } else { POSITIVE };
        if descending && !self.legacy {
            raw[0] = !raw[0];
        }
        raw[SIGN_BYTES..].copy_from_slice(&key.unsigned_abs().to_be_bytes());
        if negative != descending {
            for byte in &mut raw[SIGN_BYTES..] {
                *byte = !*byte;
            }
        }
        Ok(KEY_BYTES)
    }
    fn limit(self) -> u128 {
        10_u128.pow(u32::from(self.precision))
    }
    fn physical_key(self, coefficient: i128, descending: bool) -> i128 {
        if self.legacy && descending && coefficient < 0 {
            coefficient + LEGACY_NEGATIVE_BAND
        } else {
            coefficient
        }
    }
    pub fn value_key(self, value: &Value, descending: bool) -> Result<Option<i128>, InspectError> {
        match value {
            Value::Null => Ok(None),
            Value::Decimal(value)
                if value.scale() == self.scale
                    && value.coefficient().unsigned_abs() < self.limit() =>
            {
                Ok(Some(self.physical_key(value.coefficient(), descending)))
            }
            _ => Err(InspectError::Malformed),
        }
    }
    pub fn decode(
        self,
        bytes: &[u8],
        descending: bool,
    ) -> Result<(Option<i128>, usize), InspectError> {
        let marker = *bytes.first().ok_or(InspectError::Malformed)?;
        if marker == if descending { DESC_NULL } else { ASC_NULL } {
            return Ok((None, MARKER_BYTES));
        }
        if marker != if descending { DESC_VALUE } else { ASC_VALUE } {
            return Err(InspectError::Malformed);
        }
        let mut raw = [0; NUMERIC_BYTES];
        raw.copy_from_slice(
            bytes
                .get(MARKER_BYTES..KEY_BYTES)
                .ok_or(InspectError::Malformed)?,
        );
        // Jet 4 retains the sign byte in both directions; corrected ACE keys
        // complement it for descending order. Magnitude inversion is separate.
        if descending && !self.legacy {
            raw[0] = !raw[0];
        }
        let negative = match raw[0] {
            POSITIVE => false,
            NEGATIVE => true,
            _ => return Err(InspectError::Malformed),
        };
        if negative != descending {
            for byte in &mut raw[SIGN_BYTES..] {
                *byte = !*byte;
            }
        }
        let magnitude = u128::from_be_bytes(
            raw[SIGN_BYTES..]
                .try_into()
                .map_err(|_| InspectError::Malformed)?,
        );
        if magnitude >= self.limit() || (negative && magnitude == 0) {
            return Err(InspectError::Malformed);
        }
        let coefficient = i128::try_from(magnitude).map_err(|_| InspectError::Malformed)?;
        let coefficient = if negative { -coefficient } else { coefficient };
        // Qualified metadata bounds scale/precision; the magnitude bound proves
        // this private coefficient fits. No semantic Decimal is constructed here.
        Ok((Some(self.physical_key(coefficient, descending)), KEY_BYTES))
    }
}

pub(super) struct DecimalProfile {
    pub column: usize,
    pub descending: bool,
    pub kind: DecimalType,
}
impl KeyProfile for DecimalProfile {
    type Key = Option<i128>;
    fn components(&self) -> usize {
        1
    }
    fn max_key_bytes(&self) -> usize {
        KEY_BYTES
    }
    fn decode(
        &self,
        bytes: &[u8],
        _work: &mut KeyWork<'_>,
    ) -> Result<(Self::Key, usize), InspectError> {
        self.kind.decode(bytes, self.descending)
    }
    fn compare(&self, left: &Self::Key, right: &Self::Key) -> Ordering {
        let order = left.cmp(right);
        if self.descending {
            order.reverse()
        } else {
            order
        }
    }
    fn row_key(
        &self,
        row: &(impl KeyRow + ?Sized),
        _work: &mut KeyWork<'_>,
    ) -> Result<Self::Key, InspectError> {
        self.kind.value_key(
            row.get(self.column).ok_or(InspectError::Malformed)?,
            self.descending,
        )
    }
    fn constraint_nulls(&self, key: &Self::Key) -> Result<(bool, bool), InspectError> {
        Ok((key.is_none(), key.is_none()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tablecraft_core::values::{Currency, Double};

    #[test]
    fn descending_legacy_sign_goldens_are_not_corrected_ace_keys() {
        let legacy = DecimalType {
            precision: 28,
            scale: 4,
            legacy: true,
        };
        let corrected = DecimalType {
            legacy: false,
            ..legacy
        };
        // Literal stored keys for +1/-1. The sign byte, unlike the magnitude,
        // stays unchanged in the legacy descending representation.
        let mut positive = [0xff; 18];
        positive[0] = 0x80;
        positive[17] = 0xfe;
        let mut negative = [0; 18];
        negative[0] = 0x80;
        negative[17] = 1;
        assert_eq!(legacy.decode(&positive, true), Ok((Some(1), 18)));
        let key = legacy.decode(&negative, true).unwrap().0.unwrap();
        assert!(key > 1);
        assert_eq!(
            legacy.value_key(&Value::Decimal(Decimal::new(-1, 4).unwrap()), true),
            Ok(Some(key))
        );
        assert!(corrected.decode(&positive, true).is_err());
        assert!(corrected.decode(&negative, true).is_err());
        negative[17] = 0;
        assert_eq!(legacy.decode(&negative, true), Err(InspectError::Malformed));
        positive[1] = 0x7f;
        assert_eq!(legacy.decode(&positive, true), Err(InspectError::Malformed));
    }

    #[test]
    fn field_scale_precision_and_decimal_carrier_cannot_be_coerced() {
        let kind = DecimalType {
            precision: 4,
            scale: 2,
            legacy: false,
        };
        assert_eq!(kind.value_key(&Value::Null, false), Ok(None));
        assert_eq!(
            kind.value_key(&Value::Decimal(Decimal::new(-9999, 2).unwrap()), false),
            Ok(Some(-9999))
        );
        for value in [
            Value::Int64(1),
            Value::Currency(Currency::from_scaled(1)),
            Value::Double(Double::from_float(1.0)),
            Value::Decimal(Decimal::new(1, 0).unwrap()),
            Value::Decimal(Decimal::new(0, 0).unwrap()),
            Value::Decimal(Decimal::new(10000, 2).unwrap()),
        ] {
            assert_eq!(kind.value_key(&value, false), Err(InspectError::Malformed));
        }
    }
}

#[cfg(test)]
mod write_tests {
    use super::*;
    #[test]
    fn literal_ace_decimal_index_sign_magnitude_and_direction_goldens() {
        let kind = DecimalType {
            precision: 28,
            scale: 4,
            legacy: false,
        };
        for (coefficient, descending, expected) in [
            (
                1,
                false,
                [127, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            ),
            (
                -1,
                false,
                [
                    127, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 254,
                ],
            ),
            (
                1,
                true,
                [
                    128, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                    255, 254,
                ],
            ),
            (
                -1,
                true,
                [128, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            ),
        ] {
            let mut bytes = [0; KEY_BYTES];
            assert_eq!(
                kind.encode(Some(coefficient), descending, &mut bytes)
                    .unwrap(),
                KEY_BYTES
            );
            assert_eq!(bytes, expected);
            assert!(
                kind.encode(Some(coefficient), descending, &mut bytes[..KEY_BYTES - 1])
                    .is_err()
            );
        }
        let mut null = [0; 1];
        assert_eq!(kind.encode(None, true, &mut null).unwrap(), 1);
        assert_eq!(null, [255]);
        assert!(
            kind.encode(Some(10_i128.pow(28)), false, &mut [0; KEY_BYTES])
                .is_err()
        );
        assert!(
            DecimalType {
                legacy: true,
                ..kind
            }
            .encode(Some(-1), true, &mut [0; KEY_BYTES])
            .is_err()
        );
    }
}
