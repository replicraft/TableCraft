//! Decimal-bearing column metadata stays outside the compact component codec.
use super::{
    columns::Component,
    model::ComponentValue,
    policy::{ColumnSet, KeyComponent},
};
use crate::{
    InspectError, NativeIndex, TableSchema,
    format::{data_type, layout::INDEX_KEY_COLUMNS},
};
use tablecraft_core::values::Value;

#[derive(Clone, Copy)]
pub(in crate::table::indexes) enum DecimalComponent {
    Compact(Component),
    Decimal {
        column: usize,
        kind: super::super::decimal::DecimalType,
        descending: bool,
    },
}
impl KeyComponent for DecimalComponent {
    fn descending(self) -> bool {
        match self {
            Self::Compact(component) => component.descending(),
            Self::Decimal { descending, .. } => descending,
        }
    }
    fn max_bytes(self) -> usize {
        match self {
            Self::Compact(component) => component.max_bytes(),
            Self::Decimal { .. } => super::super::decimal::KEY_BYTES,
        }
    }
    fn decode<A: ComponentValue>(self, bytes: &[u8]) -> Result<(Option<A>, usize), InspectError> {
        match self {
            Self::Compact(component) => component.decode(bytes),
            Self::Decimal {
                kind, descending, ..
            } => {
                let (value, end) = kind.decode(bytes, descending)?;
                Ok((value.map(A::decimal).transpose()?, end))
            }
        }
    }
    fn value<A: ComponentValue>(self, row: &[Value]) -> Result<Option<A>, InspectError> {
        match self {
            Self::Compact(component) => component.value(row),
            Self::Decimal {
                column,
                kind,
                descending,
            } => kind
                .value_key(row.get(column).ok_or(InspectError::Malformed)?, descending)?
                .map(A::decimal)
                .transpose(),
        }
    }
}

pub(in crate::table::indexes) struct DecimalColumns {
    values: [DecimalComponent; INDEX_KEY_COLUMNS],
    count: usize,
}
impl DecimalColumns {
    pub(in crate::table::indexes) fn new(
        index: &NativeIndex,
        schema: &TableSchema,
    ) -> Result<Self, InspectError> {
        super::super::profile::qualify(index)?;
        if !(2..=INDEX_KEY_COLUMNS).contains(&index.columns.len()) {
            return Err(InspectError::Unsupported);
        }
        let mut values = [DecimalComponent::Compact(Component::PLACEHOLDER); INDEX_KEY_COLUMNS];
        let mut decimal = false;
        for (slot, native) in values.iter_mut().zip(&index.columns) {
            let column = schema
                .columns
                .iter()
                .position(|c| c.number() == native.number())
                .ok_or(InspectError::Malformed)?;
            let field = &schema.columns[column];
            *slot = if field.native_type() == data_type::NUMERIC {
                decimal = true;
                DecimalComponent::Decimal {
                    column,
                    kind: super::super::decimal::DecimalType::from_column(field)?,
                    descending: native.descending(),
                }
            } else {
                DecimalComponent::Compact(Component::from_column(
                    column,
                    field,
                    native.descending(),
                )?)
            };
        }
        if !decimal {
            return Err(InspectError::Unsupported);
        }
        Ok(Self {
            values,
            count: index.columns.len(),
        })
    }
}
impl ColumnSet for DecimalColumns {
    type Component = DecimalComponent;
    fn components(&self) -> &[DecimalComponent] {
        &self.values[..self.count]
    }
    fn supports<A: ComponentValue>(&self) -> bool {
        A::DECIMAL_ALLOWED
            && (A::GUID_ALLOWED
                || !self
                    .components()
                    .iter()
                    .any(|c| matches!(c,DecimalComponent::Compact(c) if c.is_guid())))
    }
}

pub(in crate::table::indexes) fn has_decimal(
    index: &NativeIndex,
    schema: &TableSchema,
) -> Result<bool, InspectError> {
    for native in &index.columns {
        let field = schema
            .columns
            .iter()
            .find(|c| c.number() == native.number())
            .ok_or(InspectError::Malformed)?;
        if field.native_type() == data_type::NUMERIC {
            return Ok(true);
        }
    }
    Ok(false)
}
