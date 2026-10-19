//! Typed numeric/GUID tuple mutation reuses the read profile's ordering and NULL policy.
use super::super::composite::ComponentValue;
use super::super::{
    NativeIndex,
    composite::{Columns, CompositeKey, CompositeProfile, MixedValue},
    key::KeyProfile,
};
use super::{MutationKey, profile::WriteProfile};
use crate::{
    EditError, InspectError, TableSchema, format::layout::INDEX_KEY_COLUMNS, header::check,
};
use std::cmp::Ordering;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(super) struct TupleProfile<A = i64> {
    read: CompositeProfile<A>,
    kinds: [MutationKey; INDEX_KEY_COLUMNS],
    descending: [bool; INDEX_KEY_COLUMNS],
    columns: [usize; INDEX_KEY_COLUMNS],
}
impl<A: WriteValue> TupleProfile<A> {
    pub fn new(
        index: &NativeIndex,
        schema: &TableSchema,
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
        let mut kinds = [MutationKey::PLACEHOLDER; INDEX_KEY_COLUMNS];
        let mut descending = [false; INDEX_KEY_COLUMNS];
        let mut columns = [0; INDEX_KEY_COLUMNS];
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
            kinds[position] = MutationKey::new(
                schema.columns[column]
                    .scalar_type()
                    .ok_or(InspectError::Unsupported)?,
            )?;
            descending[position] = key.descending();
            columns[position] = column;
        }
        Ok(Self {
            read,
            kinds,
            descending,
            columns,
        })
    }
}
impl<A: WriteValue> KeyProfile for TupleProfile<A> {
    type Key = CompositeKey<A>;
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
impl<A: WriteValue> WriteProfile for TupleProfile<A> {
    fn includes(&self, column: usize) -> bool {
        self.columns[..self.components()].contains(&column)
    }
    fn encode(&self, key: Self::Key, bytes: &mut [u8]) -> Result<usize, InspectError> {
        let mut end = 0;
        for position in 0..self.components() {
            end += self.kinds[position].encode(
                key.get(position).map(A::mixed),
                self.descending[position],
                bytes.get_mut(end..).ok_or(InspectError::Malformed)?,
            )?;
        }
        Ok(end)
    }
    fn update(
        &self,
        key: &mut Self::Key,
        column: usize,
        expected: Option<MixedValue>,
        value: Option<MixedValue>,
    ) -> Result<(), EditError> {
        let target = self.columns[..self.components()]
            .iter()
            .position(|candidate| *candidate == column)
            .ok_or(InspectError::Malformed)?;
        let expected = expected.map(A::from_mixed).transpose()?;
        let value = value.map(A::from_mixed).transpose()?;
        if key.get(target) != expected {
            return Err(EditError::Conflict);
        }
        key.set(target, value);
        Ok(())
    }
}

pub(super) trait WriteValue: ComponentValue {
    fn mixed(self) -> MixedValue;
    fn from_mixed(value: MixedValue) -> Result<Self, InspectError>;
}
impl WriteValue for i64 {
    fn mixed(self) -> MixedValue {
        MixedValue::Numeric(self)
    }
    fn from_mixed(value: MixedValue) -> Result<Self, InspectError> {
        super::kind::numeric(Some(value))?.ok_or(InspectError::Malformed)
    }
}
impl WriteValue for MixedValue {
    fn mixed(self) -> MixedValue {
        self
    }
    fn from_mixed(value: MixedValue) -> Result<Self, InspectError> {
        Ok(value)
    }
}
