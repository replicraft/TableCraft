use super::super::{
    NativeIndex, guid,
    key::{MARKER_BYTES, decode_component},
    profile::KeyType,
};
use super::model::ComponentValue;
use super::policy::{ColumnSet, KeyComponent};
use crate::table::indexes::key::KeyRow;
use crate::{InspectError, NativeColumn, TableSchema, format::layout::INDEX_KEY_COLUMNS};
use tablecraft_schema::ScalarType;

#[derive(Clone, Copy)]
enum Kind {
    Numeric(KeyType),
    Guid,
}
impl Kind {
    fn from_column(column: &NativeColumn) -> Result<Self, InspectError> {
        if column.scalar_type() == Some(ScalarType::Guid) {
            return Ok(Self::Guid);
        }
        KeyType::from_column(column).map(Self::Numeric)
    }
}

#[derive(Clone, Copy)]
pub(in crate::table::indexes) struct Component {
    column: usize,
    kind: Kind,
    pub descending: bool,
}
impl Component {
    pub(super) const PLACEHOLDER: Self = Self {
        column: 0,
        kind: Kind::Numeric(KeyType::Byte),
        descending: false,
    };
    pub(super) fn from_column(
        column: usize,
        native: &NativeColumn,
        descending: bool,
    ) -> Result<Self, InspectError> {
        Ok(Self {
            column,
            kind: Kind::from_column(native)?,
            descending,
        })
    }
    pub(super) fn is_guid(self) -> bool {
        matches!(self.kind, Kind::Guid)
    }
    pub fn max_bytes(self) -> usize {
        match self.kind {
            Kind::Numeric(kind) => MARKER_BYTES + kind.bytes(),
            Kind::Guid => guid::KEY_BYTES,
        }
    }
    pub fn decode<A: ComponentValue>(
        self,
        bytes: &[u8],
    ) -> Result<(Option<A>, usize), InspectError> {
        match self.kind {
            Kind::Numeric(kind) => {
                let (value, end) = decode_component(bytes, kind, self.descending)?;
                Ok((value.map(A::numeric), end))
            }
            Kind::Guid => {
                let (value, end) = guid::decode_component(bytes, self.descending)?;
                Ok((value.map(A::guid).transpose()?, end))
            }
        }
    }
    pub fn value<A: ComponentValue>(
        self,
        row: &(impl KeyRow + ?Sized),
    ) -> Result<Option<A>, InspectError> {
        let value = row.get(self.column).ok_or(InspectError::Malformed)?;
        match self.kind {
            Kind::Numeric(kind) => Ok(kind.key(value)?.map(A::numeric)),
            Kind::Guid => guid::value_key(value)?.map(A::guid).transpose(),
        }
    }
}

pub(in crate::table::indexes) struct Columns {
    values: [Component; INDEX_KEY_COLUMNS],
    count: usize,
}
impl Columns {
    pub(in crate::table::indexes) fn new(
        index: &NativeIndex,
        schema: &TableSchema,
    ) -> Result<Self, InspectError> {
        super::super::profile::qualify(index)?;
        if !(2..=INDEX_KEY_COLUMNS).contains(&index.columns.len()) {
            return Err(InspectError::Unsupported);
        }
        let mut values = [Component::PLACEHOLDER; INDEX_KEY_COLUMNS];
        for (slot, native) in values.iter_mut().zip(&index.columns) {
            let column = schema
                .columns
                .iter()
                .position(|column| column.number() == native.number())
                .ok_or(InspectError::Malformed)?;
            *slot = Component::from_column(column, &schema.columns[column], native.descending())?;
        }
        Ok(Self {
            values,
            count: index.columns.len(),
        })
    }
    pub(super) fn as_slice(&self) -> &[Component] {
        &self.values[..self.count]
    }
    pub(in crate::table::indexes) fn has_guid(&self) -> bool {
        self.as_slice()
            .iter()
            .any(|column| matches!(column.kind, Kind::Guid))
    }
}

impl KeyComponent for Component {
    fn descending(self) -> bool {
        self.descending
    }
    fn max_bytes(self) -> usize {
        Component::max_bytes(self)
    }
    fn decode<A: ComponentValue>(self, bytes: &[u8]) -> Result<(Option<A>, usize), InspectError> {
        Component::decode(self, bytes)
    }
    fn value<A: ComponentValue>(
        self,
        row: &(impl KeyRow + ?Sized),
    ) -> Result<Option<A>, InspectError> {
        Component::value(self, row)
    }
}
impl ColumnSet for Columns {
    type Component = Component;
    fn components(&self) -> &[Component] {
        self.as_slice()
    }
    fn supports<A: ComponentValue>(&self) -> bool {
        !self.has_guid() || A::GUID_ALLOWED
    }
}
