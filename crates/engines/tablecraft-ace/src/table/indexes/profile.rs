use super::super::schema::TableSchema;
use super::{
    Int64IndexSelection, NativeIndex, composite::CompositeProfile, constraints::IGNORE_NULLS,
    guid::GuidProfile, key::SingleKey,
};
use crate::InspectError;
use crate::NativeColumn;
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
            _ => Err(InspectError::Unsupported),
        }
    }
    pub const fn bytes(self) -> usize {
        match self {
            Self::Boolean | Self::Byte => size_of::<u8>(),
            Self::Int16 => size_of::<i16>(),
            Self::Int32 => size_of::<i32>(),
            Self::Int64 | Self::Currency => size_of::<i64>(),
        }
    }
    // One qualified carrier per physical index. Currency keeps its exact scaled
    // integer without becoming an Int64 column or entering SQL coercion rules.
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
            _ => return Err(InspectError::Malformed),
        }))
    }
}

fn single_column(index: &NativeIndex, schema: &TableSchema) -> Result<usize, InspectError> {
    qualify(index)?;
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

pub(super) fn qualify(index: &NativeIndex) -> Result<(), InspectError> {
    if index.flags & IGNORE_NULLS != 0 || index.constraints().is_none() {
        return Err(InspectError::Unsupported);
    }
    Ok(())
}

pub(super) enum IndexProfile {
    Single(SingleKey),
    Composite(CompositeProfile),
    Guid(GuidProfile),
    GuidComposite(super::composite::GuidCompositeProfile),
}
impl IndexProfile {
    pub fn new(index: &NativeIndex, schema: &TableSchema) -> Result<Self, InspectError> {
        if index.columns.len() > 1 {
            let columns = super::composite::Columns::new(index, schema)?;
            return if columns.has_guid() {
                super::composite::GuidCompositeProfile::new(columns).map(Self::GuidComposite)
            } else {
                CompositeProfile::new(columns).map(Self::Composite)
            };
        }
        let column = single_column(index, schema)?;
        if schema.columns[column].scalar_type() == Some(ScalarType::Guid) {
            return Ok(Self::Guid(GuidProfile {
                column,
                descending: index.columns[0].descending(),
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
