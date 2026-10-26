use super::super::schema::TableSchema;
use super::{
    Int64IndexSelection, NativeIndex,
    composite::CompositeProfile,
    constraints::IGNORE_NULLS,
    decimal::{DecimalIndexSelection, DecimalProfile, DecimalType},
    guid::GuidProfile,
    key::SingleKey,
};
use crate::{FileFormat, InspectError, NativeColumn, format::data_type};
use tablecraft_core::values::Value;
use tablecraft_schema::ScalarType;

// Distinct semantic types may share an encoded width. Typed ranges must still
// reject Currency when the caller requests an Int64 column.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum KeyType {
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
impl KeyType {
    pub fn from_column(column: &NativeColumn) -> Result<Self, InspectError> {
        match column.scalar_type() {
            Some(ScalarType::Boolean) => Ok(Self::Boolean),
            Some(ScalarType::Byte) => Ok(Self::Byte),
            Some(ScalarType::Int16) => Ok(Self::Int16),
            Some(ScalarType::Int32) => Ok(Self::Int32),
            Some(ScalarType::Int64) => Ok(Self::Int64),
            Some(ScalarType::Currency) => Ok(Self::Currency),
            Some(ScalarType::Single) => Ok(Self::Single),
            Some(ScalarType::Double) => Ok(Self::Double),
            Some(ScalarType::DateTime) => Ok(Self::DateTime),
            _ => Err(InspectError::Unsupported),
        }
    }
    pub const fn bytes(self) -> usize {
        match self {
            Self::Boolean | Self::Byte => size_of::<u8>(),
            Self::Int16 => size_of::<i16>(),
            Self::Int32 | Self::Single => size_of::<i32>(),
            Self::Int64 | Self::Currency | Self::Double | Self::DateTime => size_of::<i64>(),
        }
    }
    // One qualified carrier per position. Currency keeps its scale; IEEE atoms
    // keep every bit through private ordering without entering SQL coercion.
    pub fn key(self, value: &Value) -> Result<Option<i64>, InspectError> {
        Ok(Some(match (self, value) {
            (Self::Boolean, Value::Boolean(value)) => super::boolean::value_key(*value),
            (Self::Boolean, Value::Null) => return Err(InspectError::Malformed),
            (_, Value::Null) => return Ok(None),
            (Self::Byte, Value::Byte(value)) => i64::from(*value),
            (Self::Int16, Value::Int16(value)) => i64::from(*value),
            (Self::Int32, Value::Int32(value)) => i64::from(*value),
            (Self::Int64, Value::Int64(value)) => *value,
            (Self::Currency, Value::Currency(value)) => value.scaled(),
            (Self::Single, Value::Single(value)) => super::floating::single_key(value.to_bits()),
            (Self::Double, Value::Double(value)) => super::floating::double_key(value.to_bits()),
            (Self::DateTime, Value::DateTime(value)) => {
                super::floating::double_key(value.to_bits())
            }
            _ => return Err(InspectError::Malformed),
        }))
    }
}

fn single_column(
    index: &NativeIndex,
    schema: &TableSchema,
    coverage: Coverage,
) -> Result<usize, InspectError> {
    coverage.qualify(index)?;
    column_position(index, schema)
}

fn column_position(index: &NativeIndex, schema: &TableSchema) -> Result<usize, InspectError> {
    if index.columns.len() != 1 {
        return Err(InspectError::Unsupported);
    }
    let column = schema
        .columns
        .iter()
        .position(|c| c.number() == index.columns[0].number())
        .ok_or(InspectError::Malformed)?;
    Ok(column)
}

/// Physical membership never promises a complete table or grants mutation authority.
/// A separately gated DELETE may use it to qualify its explicit indexed subset;
/// ordinary read/default mutation constructors retain CompleteTable coverage.
#[derive(Clone, Copy)]
pub(super) enum Coverage {
    CompleteTable,
    PhysicalMembership,
}
impl Coverage {
    pub(super) fn qualify(self, index: &NativeIndex) -> Result<(), InspectError> {
        if index.constraints().is_none()
            || (matches!(self, Self::CompleteTable) && index.flags & IGNORE_NULLS != 0)
        {
            return Err(InspectError::Unsupported);
        }
        Ok(())
    }
}

pub(super) enum IndexProfile<'a> {
    Text(super::text::TextProfile),
    EncodedTuple(super::encoded_tuple::EncodedTupleProfile<'a>),
    Binary(super::binary::BinaryProfile<'a>),
    Single(SingleKey),
    Composite(CompositeProfile),
    Guid(GuidProfile),
    GuidComposite(super::composite::GuidCompositeProfile),
    Decimal(super::decimal::DecimalProfile),
    DecimalComposite(super::composite::DecimalCompositeProfile),
}
impl<'a> IndexProfile<'a> {
    /// A separate physical-coverage profile. Ordinary All/NULL table reads still
    /// use new(), which refuses indexes that omit entirely NULL keys.
    pub fn ignored_fixed(
        index: &NativeIndex,
        schema: &'a TableSchema,
    ) -> Result<Self, InspectError> {
        if !index
            .constraints()
            .is_some_and(|constraints| constraints.ignore_nulls())
        {
            return Err(InspectError::Unsupported);
        }
        let column = column_position(index, schema)?;
        let key_type = match (
            schema.columns[column].native_type(),
            schema.columns[column].scalar_type(),
        ) {
            (data_type::INT32, Some(ScalarType::Int32)) => KeyType::Int32,
            (data_type::INT64, Some(ScalarType::Int64))
                if matches!(
                    schema.header.format(),
                    FileFormat::Ace2016 | FileFormat::Ace2019
                ) =>
            {
                KeyType::Int64
            }
            (data_type::CURRENCY, Some(ScalarType::Currency)) => KeyType::Currency,
            _ => return Err(InspectError::Unsupported),
        };
        Ok(Self::Single(SingleKey {
            column,
            key_type,
            descending: index.columns[0].descending(),
            selection: Int64IndexSelection::All,
        }))
    }

    pub fn new(index: &NativeIndex, schema: &'a TableSchema) -> Result<Self, InspectError> {
        Self::build(index, schema, Coverage::CompleteTable)
    }

    /// Reuse qualified physical codecs for complete read-only consistency checks.
    /// Callers must independently establish the eligible sequential population.
    pub fn for_check(index: &NativeIndex, schema: &'a TableSchema) -> Result<Self, InspectError> {
        if !matches!(
            schema.header.format(),
            FileFormat::Ace2016 | FileFormat::Ace2019
        ) {
            for key in &index.columns {
                let field = schema
                    .columns
                    .iter()
                    .find(|field| field.number() == key.number())
                    .ok_or(InspectError::Malformed)?;
                if field.native_type() == data_type::INT64 {
                    return Err(InspectError::Unsupported);
                }
            }
        }
        Self::build(index, schema, Coverage::PhysicalMembership)
    }

    fn build(
        index: &NativeIndex,
        schema: &'a TableSchema,
        coverage: Coverage,
    ) -> Result<Self, InspectError> {
        if index.columns.len() > 1 {
            if super::encoded_tuple::has_variable(index, schema)? {
                return super::encoded_tuple::EncodedTupleProfile::new(index, schema, coverage)
                    .map(Self::EncodedTuple);
            }
            if super::composite::has_decimal(index, schema)? {
                let columns =
                    super::composite::DecimalColumns::with_coverage(index, schema, coverage)?;
                return super::composite::DecimalCompositeProfile::new(columns)
                    .map(Self::DecimalComposite);
            }
            let columns = super::composite::Columns::with_coverage(index, schema, coverage)?;
            return if columns.has_guid() {
                super::composite::GuidCompositeProfile::new(columns).map(Self::GuidComposite)
            } else {
                CompositeProfile::new(columns).map(Self::Composite)
            };
        }
        let column = single_column(index, schema, coverage)?;
        if schema.columns[column].native_type() == crate::format::data_type::SHORT_BINARY
            && schema.columns[column].scalar_type() == Some(ScalarType::Binary)
        {
            return Ok(Self::Binary(super::binary::BinaryProfile {
                column,
                descending: index.columns[0].descending(),
                maximum: schema.columns[column].storage_length(),
                selection: super::binary::BinaryIndexSelection::All,
            }));
        }
        if matches!(
            schema.columns[column].native_type(),
            crate::format::data_type::SHORT_TEXT | crate::format::data_type::LONG_TEXT
        ) && schema.columns[column].scalar_type() == Some(ScalarType::Text)
        {
            return super::text::TextProfile::new(
                column,
                &schema.columns[column],
                index.columns[0].descending(),
            )
            .map(Self::Text);
        }
        if schema.columns[column].scalar_type() == Some(ScalarType::Guid) {
            return Ok(Self::Guid(GuidProfile {
                column,
                selection: super::guid::GuidIndexSelection::All,
                descending: index.columns[0].descending(),
            }));
        }
        if schema.columns[column].scalar_type() == Some(ScalarType::Decimal) {
            return Ok(Self::Decimal(DecimalProfile {
                column,
                descending: index.columns[0].descending(),
                kind: DecimalType::from_column(&schema.columns[column])?,
                selection: DecimalIndexSelection::All,
            }));
        }
        let key_type = KeyType::from_column(&schema.columns[column])?;
        Ok(Self::Single(SingleKey {
            column,
            key_type,
            descending: index.columns[0].descending(),
            selection: Int64IndexSelection::All,
        }))
    }
}

#[cfg(test)]
mod tests;
