//! Closed original declaration parameters; SQL aliases and policies are absent.
use super::Error;
use crate::{
    CreateColumn,
    format::data_type::{
        BOOLEAN, BOOLEAN_BYTES, BYTE, BYTE_BYTES, CURRENCY, CURRENCY_BYTES, DATE_TIME,
        DATE_TIME_BYTES, DOUBLE, DOUBLE_BYTES, GUID, GUID_BYTES, INT16, INT16_BYTES, INT32,
        INT32_BYTES, INT64, INT64_BYTES, MAX_SHORT_TEXT_BYTES, NUMERIC, NUMERIC_BYTES, SHORT_TEXT,
        SINGLE, SINGLE_BYTES,
    },
};
use tablecraft_schema::{DecimalConstraints, ScalarType};

pub(super) struct Shape {
    pub(super) kind: u8,
    pub(super) width: u16,
    pub(super) precision: u8,
    pub(super) scale: u8,
}

impl Shape {
    pub(super) fn capture(column: CreateColumn<'_>) -> Result<Self, Error> {
        let (kind, width) = match column.scalar_type() {
            ScalarType::Boolean => (BOOLEAN, BOOLEAN_BYTES),
            ScalarType::Byte => (BYTE, BYTE_BYTES),
            ScalarType::Int16 => (INT16, INT16_BYTES),
            ScalarType::Int32 => (INT32, INT32_BYTES),
            ScalarType::Int64 => (INT64, INT64_BYTES),
            ScalarType::Single => (SINGLE, SINGLE_BYTES),
            ScalarType::Double => (DOUBLE, DOUBLE_BYTES),
            ScalarType::Currency => (CURRENCY, CURRENCY_BYTES),
            ScalarType::DateTime => (DATE_TIME, DATE_TIME_BYTES),
            ScalarType::Guid => (GUID, GUID_BYTES),
            ScalarType::Text => (
                SHORT_TEXT,
                column
                    .max_text_utf16_units()
                    .and_then(|units| units.checked_mul(size_of::<u16>()))
                    .ok_or(Error::InvalidRecord)?,
            ),
            ScalarType::Decimal => (NUMERIC, NUMERIC_BYTES),
            _ => return Err(Error::InvalidRecord),
        };
        let constraints = column.decimal_constraints();
        let shape = Self {
            kind,
            width: u16::try_from(width).map_err(|_| Error::InvalidRecord)?,
            precision: constraints.map_or(0, |value| value.precision()),
            scale: constraints.map_or(0, |value| value.scale()),
        };
        // Validate the exact emitted carrier rather than treating ScalarType as a wire tag.
        shape.column(column.name())?;
        Ok(shape)
    }

    pub(super) fn column<'a>(&self, name: &'a str) -> Result<CreateColumn<'a>, Error> {
        if self.kind == NUMERIC {
            if usize::from(self.width) != NUMERIC_BYTES {
                return Err(Error::InvalidRecord);
            }
            let constraints = DecimalConstraints::new(self.precision, self.scale)
                .map_err(|_| Error::InvalidRecord)?;
            return Ok(CreateColumn::decimal(name, constraints));
        }
        if self.precision != 0 || self.scale != 0 {
            return Err(Error::InvalidRecord);
        }
        if self.kind == SHORT_TEXT {
            let width = usize::from(self.width);
            if width == 0 || width > MAX_SHORT_TEXT_BYTES || !width.is_multiple_of(size_of::<u16>())
            {
                return Err(Error::InvalidRecord);
            }
            return CreateColumn::short_text(name, width / size_of::<u16>())
                .map_err(Error::Declaration);
        }
        let (scalar, width) = match self.kind {
            BOOLEAN => (ScalarType::Boolean, BOOLEAN_BYTES),
            BYTE => (ScalarType::Byte, BYTE_BYTES),
            INT16 => (ScalarType::Int16, INT16_BYTES),
            INT32 => (ScalarType::Int32, INT32_BYTES),
            INT64 => (ScalarType::Int64, INT64_BYTES),
            SINGLE => (ScalarType::Single, SINGLE_BYTES),
            DOUBLE => (ScalarType::Double, DOUBLE_BYTES),
            CURRENCY => (ScalarType::Currency, CURRENCY_BYTES),
            DATE_TIME => (ScalarType::DateTime, DATE_TIME_BYTES),
            GUID => (ScalarType::Guid, GUID_BYTES),
            _ => return Err(Error::InvalidRecord),
        };
        if usize::from(self.width) != width {
            return Err(Error::InvalidRecord);
        }
        Ok(CreateColumn::new(name, scalar))
    }
}
