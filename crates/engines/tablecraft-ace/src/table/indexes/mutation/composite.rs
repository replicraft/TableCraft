//! Integral tuple mutation reuses the read profile's ordering and NULL policy.
use super::super::{
    NativeIndex,
    composite::{Columns, CompositeKey, CompositeProfile},
    key::KeyProfile,
};
use super::{IntegralKey, profile::WriteProfile};
use crate::{
    EditError, InspectError, TableSchema, format::layout::INDEX_KEY_COLUMNS, header::check,
};
use std::cmp::Ordering;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(super) struct TupleProfile {
    read: CompositeProfile,
    kinds: [IntegralKey; INDEX_KEY_COLUMNS],
    descending: [bool; INDEX_KEY_COLUMNS],
    target: usize,
}
impl TupleProfile {
    pub fn new(
        index: &NativeIndex,
        schema: &TableSchema,
        selected: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        check(budget, cancelled)?;
        // Charge the read profile's column-resolution pass as well as the
        // mutation codec's explicit per-column lookups below.
        budget
            .charge(
                Resource::Steps,
                (index.columns().len() * schema.columns.len()) as u64,
            )
            .map_err(InspectError::from)?;
        let read = CompositeProfile::new(Columns::new(index, schema)?)?;
        let mut kinds = [IntegralKey::PLACEHOLDER; INDEX_KEY_COLUMNS];
        let mut descending = [false; INDEX_KEY_COLUMNS];
        let mut target = None;
        for (position, key) in index.columns().iter().enumerate() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, schema.columns.len() as u64)
                .map_err(InspectError::from)?;
            let column = schema
                .columns
                .iter()
                .position(|column| column.number() == key.number())
                .ok_or(InspectError::Malformed)?;
            kinds[position] = IntegralKey::new(
                schema.columns[column]
                    .scalar_type()
                    .ok_or(InspectError::Unsupported)?,
            )?;
            descending[position] = key.descending();
            if column == selected {
                target = Some(position);
            }
        }
        Ok(Self {
            read,
            kinds,
            descending,
            target: target.ok_or(InspectError::Malformed)?,
        })
    }
}
impl KeyProfile for TupleProfile {
    type Key = CompositeKey<i64>;
    fn components(&self) -> usize {
        self.read.components()
    }
    fn max_key_bytes(&self) -> usize {
        self.read.max_key_bytes()
    }
    fn decode(&self, bytes: &[u8]) -> Result<(Self::Key, usize), InspectError> {
        self.read.decode(bytes)
    }
    fn compare(&self, left: Self::Key, right: Self::Key) -> Ordering {
        self.read.compare(left, right)
    }
    fn row_key(&self, row: &[Value]) -> Result<Self::Key, InspectError> {
        self.read.row_key(row)
    }
    fn all_null(&self, key: Self::Key) -> bool {
        self.read.all_null(key)
    }
    fn any_null(&self, key: Self::Key) -> bool {
        self.read.any_null(key)
    }
}
impl WriteProfile for TupleProfile {
    fn encode(&self, key: Self::Key, bytes: &mut [u8]) -> Result<usize, InspectError> {
        let mut end = 0;
        for position in 0..self.components() {
            end += super::encoding::component(
                key.get(position),
                self.kinds[position],
                self.descending[position],
                bytes.get_mut(end..).ok_or(InspectError::Malformed)?,
            )?;
        }
        Ok(end)
    }
    fn update(
        &self,
        key: &mut Self::Key,
        expected: Option<i64>,
        value: Option<i64>,
    ) -> Result<(), EditError> {
        if key.get(self.target) != expected {
            return Err(EditError::Conflict);
        }
        key.set(self.target, value);
        Ok(())
    }
}
