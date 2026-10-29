//! Bind every original logical root to its projected key, including omitted rows.
use super::{super::profile::WriteProfile, reserve};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, TableSchema,
    format::{reader::Reader, row},
    table::{
        indexes::{
            key::{KeyRow, KeyWork},
            page::KeyRef,
        },
        page::{self, TableSnapshotLimits},
        snapshot,
    },
};
use std::io::Cursor;
use tablecraft_core::values::Value;

struct Projected<'a> {
    positions: &'a [usize],
    values: &'a [Value],
}

impl KeyRow for Projected<'_> {
    fn get(&self, column: usize) -> Option<&Value> {
        self.positions
            .iter()
            .position(|position| *position == column)
            .and_then(|position| self.values.get(position))
    }
}

pub(super) fn capture<P: WriteProfile>(
    image: &NativeImage,
    schema: &TableSchema,
    index: &NativeIndex,
    profile: &P,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<KeyRef<P::Key>>, EditError> {
    let mut names = Vec::new();
    reserve(&mut names, index.columns().len(), reader)?;
    for key in index.columns() {
        reader.tick()?;
        reader.work(schema.columns().len())?;
        let column = schema
            .columns()
            .iter()
            .find(|column| column.number() == key.number())
            .ok_or(InspectError::Malformed)?;
        names.push(column.name());
    }
    let (positions, columns) =
        snapshot::projected_columns(schema, &names, reader.budget, reader.cancelled)?;
    let rows = page::read_records_batched(
        &mut image.reader(),
        schema,
        &columns,
        Some(&positions),
        TableSnapshotLimits::default(),
        reader.budget,
        reader.cancelled,
    )?;
    let mut keys = Vec::new();
    reserve(&mut keys, rows.len(), reader)?;
    let mut previous = None;
    for (reference, values) in rows {
        reader.tick()?;
        if previous.is_some_and(|prior| prior >= reference) {
            return Err(EditError::Preservation);
        }
        previous = Some(reference);
        reader.work(profile.components().saturating_mul(positions.len()))?;
        let key = profile.row_key(
            &Projected {
                positions: &positions,
                values: &values,
            },
            &mut KeyWork {
                budget: reader.budget,
                cancelled: reader.cancelled,
            },
        )?;
        keys.push(KeyRef {
            key,
            row: row::reference(reference.page_number(), reference.slot_number()),
        });
    }
    Ok(keys)
}
