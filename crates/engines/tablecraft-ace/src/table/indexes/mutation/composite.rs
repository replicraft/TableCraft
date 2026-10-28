//! Typed numeric/GUID tuple mutation reuses the read profile's ordering and NULL policy.
use super::super::composite::ComponentValue;
use super::super::{
    NativeIndex,
    composite::{
        ColumnSet, Columns, CompositeKey, CompositeProfile, DecimalColumns, DecimalValue,
        MixedValue,
    },
    key::KeyProfile,
    profile::Coverage,
};
use super::{MutationKey, profile::WriteProfile};
use crate::table::indexes::key::KeyRow;
use crate::table::indexes::key::KeyWork;
use crate::{
    EditError, InspectError, TableSchema, format::layout::INDEX_KEY_COLUMNS, header::check,
};
use std::cmp::Ordering;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) struct TupleProfile<A = i64, C = Columns> {
    read: CompositeProfile<A, C>,
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
        Self::with_coverage(index, schema, Coverage::CompleteTable, budget, cancelled)
    }

    #[cfg(feature = "ace16-write-preview")]
    pub fn new_for_delete(
        index: &NativeIndex,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::with_coverage(
            index,
            schema,
            Coverage::PhysicalMembership,
            budget,
            cancelled,
        )
    }

    pub(super) fn with_coverage(
        index: &NativeIndex,
        schema: &TableSchema,
        coverage: Coverage,
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
        let columns = match coverage {
            Coverage::CompleteTable => Columns::new(index, schema)?,
            Coverage::PhysicalMembership => Columns::with_coverage(index, schema, coverage)?,
        };
        let read = CompositeProfile::new(columns)?;
        Self::with_read(read, index, schema, budget, cancelled)
    }
}
impl TupleProfile<DecimalValue, DecimalColumns> {
    pub fn new_decimal(
        index: &NativeIndex,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::with_decimal_coverage(index, schema, Coverage::CompleteTable, budget, cancelled)
    }

    #[cfg(feature = "ace16-write-preview")]
    pub fn new_decimal_for_delete(
        index: &NativeIndex,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::with_decimal_coverage(
            index,
            schema,
            Coverage::PhysicalMembership,
            budget,
            cancelled,
        )
    }

    fn with_decimal_coverage(
        index: &NativeIndex,
        schema: &TableSchema,
        coverage: Coverage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        check(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                (index.columns().len() * schema.columns.len()) as u64,
            )
            .map_err(InspectError::from)?;
        let columns = match coverage {
            Coverage::CompleteTable => DecimalColumns::new(index, schema)?,
            Coverage::PhysicalMembership => DecimalColumns::with_coverage(index, schema, coverage)?,
        };
        let read = CompositeProfile::new(columns)?;
        Self::with_read(read, index, schema, budget, cancelled)
    }
}
impl<A: WriteValue, C: ColumnSet> TupleProfile<A, C> {
    fn with_read(
        read: CompositeProfile<A, C>,
        index: &NativeIndex,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
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
            kinds[position] = MutationKey::from_column(&schema.columns[column])?;
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
impl<A: WriteValue, C: ColumnSet> KeyProfile for TupleProfile<A, C> {
    type Key = CompositeKey<A>;
    fn components(&self) -> usize {
        self.read.components()
    }
    fn max_key_bytes(&self) -> usize {
        self.read.max_key_bytes()
    }
    fn decode(
        &self,
        bytes: &[u8],
        work: &mut KeyWork<'_>,
    ) -> Result<(Self::Key, usize), InspectError> {
        self.read.decode(bytes, work)
    }
    fn compare(&self, left: &Self::Key, right: &Self::Key) -> Ordering {
        self.read.compare(left, right)
    }
    fn row_key(
        &self,
        row: &(impl KeyRow + ?Sized),
        work: &mut KeyWork<'_>,
    ) -> Result<Self::Key, InspectError> {
        self.read.row_key(row, work)
    }
    fn constraint_nulls(&self, key: &Self::Key) -> Result<(bool, bool), InspectError> {
        self.read.constraint_nulls(key)
    }
}
impl<A: WriteValue, C: ColumnSet> WriteProfile for TupleProfile<A, C> {
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
        expected: Option<DecimalValue>,
        value: Option<DecimalValue>,
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
    fn mixed(self) -> DecimalValue;
    fn from_mixed(value: DecimalValue) -> Result<Self, InspectError>;
}
impl WriteValue for i64 {
    fn mixed(self) -> DecimalValue {
        DecimalValue::Numeric(self)
    }
    fn from_mixed(value: DecimalValue) -> Result<Self, InspectError> {
        super::kind::numeric(Some(value))?.ok_or(InspectError::Malformed)
    }
}
impl WriteValue for MixedValue {
    fn mixed(self) -> DecimalValue {
        match self {
            Self::Numeric(v) => DecimalValue::Numeric(v),
            Self::Guid(v) => DecimalValue::Guid(v),
        }
    }
    fn from_mixed(value: DecimalValue) -> Result<Self, InspectError> {
        match value {
            DecimalValue::Numeric(v) => Ok(Self::Numeric(v)),
            DecimalValue::Guid(v) => Ok(Self::Guid(v)),
            _ => Err(InspectError::Malformed),
        }
    }
}
impl WriteValue for DecimalValue {
    fn mixed(self) -> Self {
        self
    }
    fn from_mixed(value: Self) -> Result<Self, InspectError> {
        Ok(value)
    }
}
