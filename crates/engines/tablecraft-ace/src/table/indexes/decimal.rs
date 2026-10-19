//! ACE 16 fixed-point index keys; exact field scale stays separate from SQL coercion.
use super::{
    key::{ASC_NULL, ASC_VALUE, DESC_NULL, DESC_VALUE, KeyProfile, MARKER_BYTES},
    page::{CHILD_BYTES, ROW_BYTES},
};
use crate::{InspectError, NativeColumn, ReadProfile, format::data_type::NUMERIC_BYTES};
use std::cmp::Ordering;
use tablecraft_core::values::Value;
use tablecraft_schema::ScalarType;

const POSITIVE: u8 = 0xff;
const NEGATIVE: u8 = 0x00;
const SIGN_BYTES: usize = size_of::<u8>();
pub(super) const KEY_BYTES: usize = MARKER_BYTES + NUMERIC_BYTES;
pub(super) const MAX_ENTRY_BYTES: usize = KEY_BYTES + ROW_BYTES + CHILD_BYTES;

#[derive(Clone, Copy)]
pub(super) struct DecimalType {
    precision: u8,
    scale: u8,
}
impl DecimalType {
    pub fn from_column(column: &NativeColumn) -> Result<Self, InspectError> {
        if !matches!(
            column.read_profile(),
            ReadProfile::Ace2016 | ReadProfile::Ace2019
        ) || column.scalar_type() != Some(ScalarType::Decimal)
        {
            return Err(InspectError::Unsupported);
        }
        let (precision, scale) = column.decimal_parameters().ok_or(InspectError::Malformed)?;
        Ok(Self { precision, scale })
    }
    fn limit(self) -> u128 {
        10_u128.pow(u32::from(self.precision))
    }
    pub fn value_key(self, value: &Value) -> Result<Option<i128>, InspectError> {
        match value {
            Value::Null => Ok(None),
            Value::Decimal(value)
                if value.scale() == self.scale
                    && value.coefficient().unsigned_abs() < self.limit() =>
            {
                Ok(Some(value.coefficient()))
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
        if descending {
            for byte in &mut raw {
                *byte = !*byte;
            }
        }
        let negative = match raw[0] {
            POSITIVE => false,
            NEGATIVE => true,
            _ => return Err(InspectError::Malformed),
        };
        if negative {
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
        Ok((Some(coefficient), KEY_BYTES))
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
    fn decode(&self, bytes: &[u8]) -> Result<(Self::Key, usize), InspectError> {
        self.kind.decode(bytes, self.descending)
    }
    fn compare(&self, left: Self::Key, right: Self::Key) -> Ordering {
        let order = left.cmp(&right);
        if self.descending {
            order.reverse()
        } else {
            order
        }
    }
    fn row_key(&self, row: &[Value]) -> Result<Self::Key, InspectError> {
        self.kind
            .value_key(row.get(self.column).ok_or(InspectError::Malformed)?)
    }
    fn all_null(&self, key: Self::Key) -> bool {
        key.is_none()
    }
    fn any_null(&self, key: Self::Key) -> bool {
        key.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tablecraft_core::values::{Currency, Decimal, Double};

    #[test]
    fn field_scale_precision_and_decimal_carrier_cannot_be_coerced() {
        let kind = DecimalType {
            precision: 4,
            scale: 2,
        };
        assert_eq!(kind.value_key(&Value::Null), Ok(None));
        assert_eq!(
            kind.value_key(&Value::Decimal(Decimal::new(-9999, 2).unwrap())),
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
            assert_eq!(kind.value_key(&value), Err(InspectError::Malformed));
        }
    }
}
