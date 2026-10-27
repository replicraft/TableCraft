//! Owned variable-width physical tuples. Digest the complete tuple only once.
mod component;
use super::{
    NativeIndex,
    key::{KeyProfile, KeyRow, KeyWork},
    page::{CHILD_BYTES, ROW_BYTES},
    profile::Coverage,
};
use crate::{
    InspectError, TableSchema,
    format::{
        data_type,
        layout::{INDEX_KEY_BYTES, INDEX_KEY_COLUMNS},
    },
    header::check,
};
use component::{Component, Kind};
use std::cmp::Ordering;
use tablecraft_core::{limits::Resource, values::Value};

pub(super) const MAX_ENTRY_BYTES: usize = INDEX_KEY_BYTES + ROW_BYTES + CHILD_BYTES;

pub(in crate::table::indexes) struct EncodedTupleProfile<'a> {
    // Metadata is bounded by the native ten-column ceiling, without heap clones.
    columns: [Option<Component<'a>>; INDEX_KEY_COLUMNS],
    count: usize,
    memo: bool,
}
/// Physical equality ignores row-only NULL metadata. Decoded separator keys may
/// have been cut inside a component; they cannot supply constraint NULL facts.
pub(in crate::table::indexes) struct EncodedTupleKey {
    bytes: Vec<u8>,
    row_nulls: Option<u16>,
}
#[cfg(feature = "ace16-write-preview")]
impl EncodedTupleKey {
    pub(in crate::table::indexes) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

impl PartialEq for EncodedTupleKey {
    fn eq(&self, other: &Self) -> bool {
        self.bytes == other.bytes
    }
}
impl Eq for EncodedTupleKey {}

pub(in crate::table::indexes) fn has_variable(
    index: &NativeIndex,
    schema: &TableSchema,
) -> Result<bool, InspectError> {
    let mut found = false;
    for column in &index.columns {
        let field = schema
            .columns
            .iter()
            .find(|field| field.number() == column.number())
            .ok_or(InspectError::Malformed)?;
        found |= matches!(
            field.native_type(),
            data_type::SHORT_TEXT | data_type::LONG_TEXT | data_type::SHORT_BINARY
        );
    }
    Ok(found)
}
impl<'a> EncodedTupleProfile<'a> {
    pub fn new(
        index: &NativeIndex,
        schema: &'a TableSchema,
        coverage: Coverage,
    ) -> Result<Self, InspectError> {
        coverage.qualify(index)?;
        if !(2..=INDEX_KEY_COLUMNS).contains(&index.columns.len()) {
            return Err(InspectError::Unsupported);
        }
        let mut columns = std::array::from_fn(|_| None);
        let mut memo = false;
        let mut variable = false;
        for (slot, column) in columns.iter_mut().zip(&index.columns) {
            let position = schema
                .columns
                .iter()
                .position(|field| field.number() == column.number())
                .ok_or(InspectError::Malformed)?;
            let component =
                Component::new(position, &schema.columns[position], column.descending())?;
            if let Kind::Text(field) = &component.kind {
                variable = true;
                memo |= field.native_type() == data_type::LONG_TEXT;
            }
            variable |= matches!(component.kind, Kind::Binary(_));
            *slot = Some(component);
        }
        if !variable {
            return Err(InspectError::Unsupported);
        }
        Ok(Self {
            columns,
            count: index.columns.len(),
            memo,
        })
    }
    fn columns(&self) -> impl Iterator<Item = &Component<'_>> {
        self.columns[..self.count].iter().flatten()
    }
    fn null_mask(&self) -> u16 {
        (1 << self.count) - 1
    }
}
impl KeyProfile for EncodedTupleProfile<'_> {
    type Key = EncodedTupleKey;
    fn components(&self) -> usize {
        self.count
    }
    fn max_key_bytes(&self) -> usize {
        INDEX_KEY_BYTES
    }
    fn decode(
        &self,
        bytes: &[u8],
        work: &mut KeyWork<'_>,
    ) -> Result<(Self::Key, usize), InspectError> {
        check(work.budget, work.cancelled)?;
        let first = self.columns().next().ok_or(InspectError::Malformed)?;
        let marker = *bytes.first().ok_or(InspectError::Malformed)?;
        if bytes.len() < self.count
            || bytes.len() > INDEX_KEY_BYTES
            || (marker != first.marker(true) && marker != first.marker(false))
        {
            return Err(InspectError::Malformed);
        }
        // Entry framing already excludes row/child trailers. Complete row-key
        // re-encoding validates every leaf component, including unprojected ones.
        let mut owned = Vec::new();
        append(&mut owned, bytes, work)?;
        Ok((
            EncodedTupleKey {
                bytes: owned,
                row_nulls: None,
            },
            bytes.len(),
        ))
    }
    fn compare(&self, left: &Self::Key, right: &Self::Key) -> Ordering {
        left.bytes.cmp(&right.bytes)
    }
    fn row_key(
        &self,
        row: &(impl KeyRow + ?Sized),
        work: &mut KeyWork<'_>,
    ) -> Result<Self::Key, InspectError> {
        let mut bytes = Vec::new();
        let mut nulls = 0;
        for (position, column) in self.columns().enumerate() {
            if matches!(
                row.get(column.column).ok_or(InspectError::Malformed)?,
                Value::Null
            ) {
                nulls |= 1 << position;
            }
            column.append(row, &mut bytes, work)?;
        }
        super::entry_digest::truncate(&mut bytes, work)?;
        check(work.budget, work.cancelled)?;
        Ok(EncodedTupleKey {
            bytes,
            row_nulls: Some(nulls),
        })
    }

    fn constraint_nulls(&self, key: &Self::Key) -> Result<(bool, bool), InspectError> {
        let nulls = key.row_nulls.ok_or(InspectError::Malformed)?;
        Ok((nulls == self.null_mask(), nulls != 0))
    }
    fn unique_key_is_exact(&self, key: &Self::Key) -> bool {
        !self.memo && key.bytes.len() < INDEX_KEY_BYTES
    }
}
fn append(output: &mut Vec<u8>, bytes: &[u8], work: &mut KeyWork<'_>) -> Result<(), InspectError> {
    check(work.budget, work.cancelled)?;
    work.budget.charge(Resource::Steps, bytes.len() as u64)?;
    work.budget
        .charge_allocation(bytes.len() as u64, size_of::<u8>() as u64)?;
    output
        .try_reserve_exact(bytes.len())
        .map_err(|_| InspectError::Allocation)?;
    output.extend_from_slice(bytes);
    Ok(())
}

#[cfg(test)]
mod tests;
