//! Bounded exact numeric tuples in native per-component index order.
use super::{
    NativeIndex,
    key::{KeyProfile, MARKER_BYTES, decode_component},
    page::{CHILD_BYTES, ROW_BYTES},
    profile::KeyType,
};
use crate::{InspectError, TableSchema, format::layout::INDEX_KEY_COLUMNS};
use std::cmp::Ordering;
use tablecraft_core::values::Value;

pub(super) const MAX_ENTRY_BYTES: usize =
    INDEX_KEY_COLUMNS * (MARKER_BYTES + size_of::<i64>()) + ROW_BYTES + CHILD_BYTES;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct CompositeKey {
    values: [i64; INDEX_KEY_COLUMNS],
    present: u16,
}
impl CompositeKey {
    fn empty() -> Self {
        Self {
            values: [0; INDEX_KEY_COLUMNS],
            present: 0,
        }
    }
    fn set(&mut self, position: usize, value: Option<i64>) {
        if let Some(value) = value {
            self.values[position] = value;
            self.present |= 1 << position;
        }
    }
    fn get(self, position: usize) -> Option<i64> {
        (self.present & (1 << position) != 0).then_some(self.values[position])
    }
}

#[derive(Clone, Copy)]
struct Component {
    column: usize,
    key_type: KeyType,
    descending: bool,
}

pub(super) struct CompositeProfile {
    components: [Component; INDEX_KEY_COLUMNS],
    count: usize,
}
impl CompositeProfile {
    pub fn new(index: &NativeIndex, schema: &TableSchema) -> Result<Self, InspectError> {
        super::profile::qualify(index)?;
        if !(2..=INDEX_KEY_COLUMNS).contains(&index.columns.len()) {
            return Err(InspectError::Unsupported);
        }
        let mut components = [Component {
            column: 0,
            key_type: KeyType::Byte,
            descending: false,
        }; INDEX_KEY_COLUMNS];
        for (slot, native) in components.iter_mut().zip(&index.columns) {
            let column = schema
                .columns
                .iter()
                .position(|column| column.number() == native.number())
                .ok_or(InspectError::Malformed)?;
            *slot = Component {
                column,
                key_type: KeyType::from_column(&schema.columns[column])?,
                descending: native.descending(),
            };
        }
        Ok(Self {
            components,
            count: index.columns.len(),
        })
    }
    fn columns(&self) -> &[Component] {
        &self.components[..self.count]
    }
}

impl KeyProfile for CompositeProfile {
    type Key = CompositeKey;
    fn components(&self) -> usize {
        self.count
    }
    fn max_key_bytes(&self) -> usize {
        self.columns()
            .iter()
            .map(|component| MARKER_BYTES + component.key_type.bytes())
            .sum()
    }
    fn decode(&self, bytes: &[u8]) -> Result<(Self::Key, usize), InspectError> {
        let mut key = CompositeKey::empty();
        let mut end = 0;
        for (position, component) in self.columns().iter().enumerate() {
            let (value, consumed) = decode_component(
                bytes.get(end..).ok_or(InspectError::Malformed)?,
                component.key_type,
                component.descending,
            )?;
            key.set(position, value);
            end += consumed;
        }
        Ok((key, end))
    }
    fn compare(&self, left: Self::Key, right: Self::Key) -> Ordering {
        for (position, component) in self.columns().iter().enumerate() {
            let order = left.get(position).cmp(&right.get(position));
            if order != Ordering::Equal {
                return if component.descending {
                    order.reverse()
                } else {
                    order
                };
            }
        }
        Ordering::Equal
    }
    fn row_key(&self, row: &[Value]) -> Result<Self::Key, InspectError> {
        let mut key = CompositeKey::empty();
        for (position, component) in self.columns().iter().enumerate() {
            key.set(
                position,
                component
                    .key_type
                    .key(row.get(component.column).ok_or(InspectError::Malformed)?)?,
            );
        }
        Ok(key)
    }
    fn all_null(&self, key: Self::Key) -> bool {
        key.present == 0
    }
    fn any_null(&self, key: Self::Key) -> bool {
        key.present != (1 << self.count) - 1
    }
}
