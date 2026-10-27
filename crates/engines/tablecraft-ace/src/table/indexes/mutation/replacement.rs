//! Genuine owned-key replacements for variable-slot UPDATE previews.
mod qualification;
use super::super::{
    IndexCheck, NativeIndex, binary, encoded_tuple,
    key::{KeyProfile, KeyRow, KeyWork},
    profile::Coverage,
    text,
};
use super::{IndexDataPatch, retained::Preserved, tree::Tree};
use crate::{
    EditError, InspectError, NativeImage, TableSchema, WriteBlocker,
    format::{
        data_type::{SHORT_BINARY, SHORT_TEXT},
        reader::Reader,
    },
    header::check,
};
pub(in crate::table) use qualification::qualify;
use std::cmp::Ordering;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(in crate::table) struct SlotKeyUpdate<'a> {
    pub reference: u32,
    pub columns: &'a [usize],
    pub before: &'a [Value],
    pub column: usize,
    pub value: &'a Value,
}

impl KeyRow for SlotKeyUpdate<'_> {
    fn get(&self, column: usize) -> Option<&Value> {
        if column == self.column {
            Some(self.value)
        } else {
            self.columns
                .iter()
                .position(|position| *position == column)
                .and_then(|position| self.before.get(position))
        }
    }
}

struct Original<'a>(&'a SlotKeyUpdate<'a>);

impl KeyRow for Original<'_> {
    fn get(&self, column: usize) -> Option<&Value> {
        self.0
            .columns
            .iter()
            .position(|position| *position == column)
            .and_then(|position| self.0.before.get(position))
    }
}

trait ReplacementProfile: KeyProfile {
    fn encoded_row(
        &self,
        row: &(impl KeyRow + ?Sized),
        work: &mut KeyWork<'_>,
    ) -> Result<Vec<u8>, InspectError>;
}

impl ReplacementProfile for text::TextProfile {
    fn encoded_row(
        &self,
        row: &(impl KeyRow + ?Sized),
        work: &mut KeyWork<'_>,
    ) -> Result<Vec<u8>, InspectError> {
        self.row_key(row, work).map(|key| key.into_bytes())
    }
}

impl ReplacementProfile for encoded_tuple::EncodedTupleProfile<'_> {
    fn encoded_row(
        &self,
        row: &(impl KeyRow + ?Sized),
        work: &mut KeyWork<'_>,
    ) -> Result<Vec<u8>, InspectError> {
        self.row_key(row, work).map(|key| key.into_bytes())
    }
}

impl ReplacementProfile for binary::BinaryProfile<'_> {
    fn encoded_row(
        &self,
        row: &(impl KeyRow + ?Sized),
        work: &mut KeyWork<'_>,
    ) -> Result<Vec<u8>, InspectError> {
        let mut encoded = [0; binary::MAX_ENTRY_BYTES];
        let length = binary::encode_component(
            row.get(self.column).ok_or(InspectError::Malformed)?,
            self.maximum,
            self.descending,
            &mut encoded,
        )?;
        check(work.budget, work.cancelled)?;
        work.budget.charge_allocation(length as u64, 1)?;
        work.budget.charge(Resource::Steps, length as u64)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| InspectError::Allocation)?;
        bytes.extend_from_slice(&encoded[..length]);
        Ok(bytes)
    }
}

pub(in crate::table) fn patches(
    image: &NativeImage,
    schema: &TableSchema,
    indexes: &[IndexCheck],
    column: usize,
    updates: &[SlotKeyUpdate<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<IndexDataPatch>, EditError> {
    qualify(schema, indexes, column, budget, cancelled)?;

    let mut patches = Vec::new();
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    for (position, checked) in indexes.iter().enumerate() {
        reader.tick()?;
        let index = checked.index();
        if !index
            .columns()
            .iter()
            .any(|key| key.number() == schema.columns[column].number())
        {
            continue;
        }
        reader.work(indexes.len())?;
        if indexes[..position]
            .iter()
            .any(|previous| previous.index().data_number() == index.data_number())
        {
            continue;
        }

        if index.columns().len() == 1 {
            let metadata = &schema.columns[column];
            match metadata.native_type() {
                SHORT_TEXT => edit::<_, { text::MAX_ENTRY_BYTES }>(
                    image,
                    &mut reader,
                    schema.definition,
                    index,
                    text::TextProfile::new(column, metadata, index.columns()[0].descending())?,
                    updates,
                    &mut patches,
                )?,
                SHORT_BINARY => edit::<_, { binary::MAX_ENTRY_BYTES }>(
                    image,
                    &mut reader,
                    schema.definition,
                    index,
                    binary::BinaryProfile {
                        column,
                        descending: index.columns()[0].descending(),
                        maximum: metadata.storage_length(),
                        selection: binary::BinaryIndexSelection::All,
                    },
                    updates,
                    &mut patches,
                )?,
                _ => return Err(EditError::Unsupported(WriteBlocker::IndexedColumn)),
            }
        } else {
            let profile =
                encoded_tuple::EncodedTupleProfile::new(index, schema, Coverage::CompleteTable)?;
            edit::<_, { encoded_tuple::MAX_ENTRY_BYTES }>(
                image,
                &mut reader,
                schema.definition,
                index,
                profile,
                updates,
                &mut patches,
            )?;
        }
    }
    Ok(patches)
}

fn edit<P: ReplacementProfile, const N: usize>(
    image: &NativeImage,
    reader: &mut Reader<'_, std::io::Cursor<&[u8]>>,
    owner: u32,
    index: &NativeIndex,
    original: P,
    updates: &[SlotKeyUpdate<'_>],
    patches: &mut Vec<IndexDataPatch>,
) -> Result<(), EditError> {
    let profile = Preserved(original);
    let mut entries = Vec::new();
    let tree = Tree::read::<_, N>(
        image,
        reader,
        owner,
        index.root_page(),
        &profile,
        0,
        &mut entries,
    )?;
    let mut matched = 0;
    for entry in &mut entries {
        reader.tick()?;
        reader.work(updates.len())?;
        if let Some(update) = updates.iter().find(|update| update.reference == entry.row) {
            reader.work(2 * profile.components() * update.columns.len())?;
            let mut work = KeyWork {
                budget: reader.budget,
                cancelled: reader.cancelled,
            };
            let before = profile.row_key(&Original(update), &mut work)?;
            if profile.compare(&entry.key, &before) != Ordering::Equal {
                return Err(EditError::Conflict);
            }
            let wire = profile.0.encoded_row(update, &mut work)?;
            entry.key = profile.decode(&wire, &mut work)?.0;
            matched += 1;
        }
    }
    if matched != updates.len() {
        return Err(EditError::Preservation);
    }
    if updates.is_empty() {
        return Ok(());
    }

    let levels = entries.len().checked_ilog2().unwrap_or(0) as usize + 1;
    reader.work(entries.len() * levels * profile.max_key_bytes())?;
    entries.sort_unstable_by(|left, right| left.compare_with(right, &profile));
    let mut remaining = entries.as_slice();
    tree.replace::<_, N>(
        image,
        &mut remaining,
        &profile,
        patches,
        reader.budget,
        reader.cancelled,
    )?;
    if !remaining.is_empty() {
        return Err(EditError::Preservation);
    }
    // Full after-image checks validate actual row NULL facts, uniqueness,
    // required fields and every reconstructed key before publication.
    Ok(())
}
