use super::super::{
    guid,
    key::{KeyProfile, MARKER_BYTES},
    page::{CHILD_BYTES, ROW_BYTES},
};
use super::{
    columns::Columns,
    model::{ComponentValue, CompositeKey},
    policy::{ColumnSet, KeyComponent},
};
use crate::table::indexes::key::KeyRow;
use crate::{InspectError, format::layout::INDEX_KEY_COLUMNS};
use std::{cmp::Ordering, marker::PhantomData};

pub(in crate::table::indexes) const MAX_ENTRY_BYTES: usize =
    INDEX_KEY_COLUMNS * (MARKER_BYTES + size_of::<i64>()) + ROW_BYTES + CHILD_BYTES;
pub(in crate::table::indexes) const MAX_GUID_ENTRY_BYTES: usize =
    INDEX_KEY_COLUMNS * guid::KEY_BYTES + ROW_BYTES + CHILD_BYTES;

pub(in crate::table::indexes) struct CompositeProfile<A = i64, C = Columns> {
    columns: C,
    atom: PhantomData<A>,
}
impl<A: ComponentValue, C: ColumnSet> CompositeProfile<A, C> {
    pub(in crate::table::indexes) fn new(columns: C) -> Result<Self, InspectError> {
        if !columns.supports::<A>() {
            return Err(InspectError::Unsupported);
        }
        Ok(Self {
            columns,
            atom: PhantomData,
        })
    }
}
impl<A: ComponentValue, C: ColumnSet> KeyProfile for CompositeProfile<A, C> {
    type Key = CompositeKey<A>;
    fn components(&self) -> usize {
        self.columns.components().len()
    }
    fn max_key_bytes(&self) -> usize {
        self.columns
            .components()
            .iter()
            .map(|column| column.max_bytes())
            .sum()
    }
    fn decode(&self, bytes: &[u8]) -> Result<(Self::Key, usize), InspectError> {
        let mut key = CompositeKey::empty();
        let mut end = 0;
        for (position, component) in self.columns.components().iter().enumerate() {
            let (value, consumed) =
                component.decode(bytes.get(end..).ok_or(InspectError::Malformed)?)?;
            key.set(position, value);
            end += consumed;
        }
        Ok((key, end))
    }
    fn compare(&self, left: &Self::Key, right: &Self::Key) -> Ordering {
        for (position, component) in self.columns.components().iter().enumerate() {
            let order = left.get(position).cmp(&right.get(position));
            if order != Ordering::Equal {
                return if component.descending() {
                    order.reverse()
                } else {
                    order
                };
            }
        }
        Ordering::Equal
    }
    fn row_key(&self, row: &(impl KeyRow + ?Sized)) -> Result<Self::Key, InspectError> {
        let mut key = CompositeKey::empty();
        for (position, component) in self.columns.components().iter().enumerate() {
            key.set(position, component.value(row)?);
        }
        Ok(key)
    }
    fn all_null(&self, key: &Self::Key) -> bool {
        key.all_null()
    }
    fn any_null(&self, key: &Self::Key) -> bool {
        key.any_null(self.components())
    }
}
