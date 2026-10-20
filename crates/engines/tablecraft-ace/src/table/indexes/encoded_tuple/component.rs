//! Complete column encodings, before entry-level truncation.
use super::super::{
    binary,
    decimal::DecimalType,
    guid,
    key::{ASC_NULL, ASC_VALUE, DESC_NULL, DESC_VALUE, KeyRow, KeyWork},
    numeric::{NumericKey, encode_component},
    profile::KeyType,
    text::TextProfile,
};
use crate::{InspectError, NativeColumn, format::data_type, header::check};
use tablecraft_schema::ScalarType;

const SCALAR_BUFFER_BYTES: usize = binary::MAX_ENTRY_BYTES;

pub(super) enum Kind<'a> {
    Text(&'a NativeColumn),
    Numeric(KeyType, NumericKey),
    Guid,
    Binary(usize),
    Decimal(DecimalType),
}
pub(super) struct Component<'a> {
    pub(super) column: usize,
    pub(super) descending: bool,
    pub(super) kind: Kind<'a>,
}
impl<'a> Component<'a> {
    pub(super) fn new(
        column: usize,
        field: &'a NativeColumn,
        descending: bool,
    ) -> Result<Self, InspectError> {
        let kind = match field.scalar_type().ok_or(InspectError::Unsupported)? {
            ScalarType::Text
                if matches!(
                    field.native_type(),
                    data_type::SHORT_TEXT | data_type::LONG_TEXT
                ) =>
            {
                TextProfile::new(column, field, descending)?;
                Kind::Text(field)
            }
            ScalarType::Guid => Kind::Guid,
            ScalarType::Binary if field.native_type() == data_type::SHORT_BINARY => {
                Kind::Binary(field.storage_length())
            }
            ScalarType::Decimal => Kind::Decimal(DecimalType::from_column(field)?),
            kind => Kind::Numeric(
                KeyType::from_column(field)?,
                NumericKey::new(kind).map_err(|_| InspectError::Unsupported)?,
            ),
        };
        Ok(Self {
            column,
            descending,
            kind,
        })
    }
    pub(super) fn marker(&self, null: bool) -> u8 {
        match (null, self.descending) {
            (true, false) => ASC_NULL,
            (true, true) => DESC_NULL,
            (false, false) => ASC_VALUE,
            (false, true) => DESC_VALUE,
        }
    }
    pub(super) fn append(
        &self,
        row: &(impl KeyRow + ?Sized),
        output: &mut Vec<u8>,
        work: &mut KeyWork<'_>,
    ) -> Result<(), InspectError> {
        check(work.budget, work.cancelled)?;
        let value = row.get(self.column).ok_or(InspectError::Malformed)?;
        let mut fixed = [0; SCALAR_BUFFER_BYTES];
        let length = match &self.kind {
            Kind::Text(field) => {
                let profile = TextProfile::new(self.column, field, self.descending)?;
                let bytes = profile.row_component(row, work)?;
                return super::append(output, &bytes, work);
            }
            Kind::Numeric(kind, codec) => {
                encode_component(kind.key(value)?, *codec, self.descending, &mut fixed)?
            }
            Kind::Binary(maximum) => {
                binary::encode_component(value, *maximum, self.descending, &mut fixed)?
            }
            Kind::Guid => {
                guid::encode_component(guid::value_key(value)?, self.descending, &mut fixed)?
            }
            Kind::Decimal(kind) => {
                kind.encode_read_component(value, self.descending, &mut fixed)?
            }
        };
        super::append(output, &fixed[..length], work)
    }
}
