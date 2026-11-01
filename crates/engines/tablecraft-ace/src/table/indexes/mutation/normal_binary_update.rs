//! Fitting Binary leaf replacements below fresh normal UPDATE admission.
use super::{IndexDataPatch, leaf_removal, repack, retained::Preserved};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, TableSchema, WriteBlocker,
    format::{page::PAGE_BYTES, reader::Reader},
    header::check,
    table::indexes::{
        binary::{self, BinaryIndexSelection, BinaryProfile},
        key::{KeyProfile, KeyRow, KeyWork},
        page,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

pub(in crate::table) struct Change<'value> {
    pub reference: u32,
    pub expected: &'value Value,
    pub value: &'value Value,
}

struct Cell<'value> {
    column: usize,
    value: &'value Value,
}

impl KeyRow for Cell<'_> {
    fn get(&self, column: usize) -> Option<&Value> {
        (column == self.column).then_some(self.value)
    }
}

pub(in crate::table) fn patches(
    image: &NativeImage,
    schema: &TableSchema,
    index: &NativeIndex,
    changes: &[Change<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<IndexDataPatch>, EditError> {
    check(budget, cancelled)?;
    let [key] = index.columns() else {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    };
    let column = key.number();
    let field = schema
        .columns()
        .get(column)
        .ok_or(EditError::Preservation)?;
    let profile = Preserved(BinaryProfile {
        column,
        descending: key.descending(),
        maximum: field.storage_length(),
        selection: BinaryIndexSelection::All,
    });
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let mut root = page::read_profile::<_, _, { binary::MAX_ENTRY_BYTES }>(
        &mut reader,
        index.root_page(),
        schema.definition_page(),
        &profile,
    )?;
    let base = index.root_page() as usize * PAGE_BYTES;
    let before = image
        .as_bytes()
        .get(base..base + PAGE_BYTES)
        .ok_or(EditError::Preservation)?;
    leaf_removal::require_root(&root, before)?;
    let mut matched = 0_usize;
    for entry in &mut root.entries {
        reader.tick()?;
        reader.work(changes.len())?;
        let Some(change) = changes
            .iter()
            .find(|change| change.reference == entry.value.row)
        else {
            continue;
        };
        // This physical pass independently reconstructs and compares the old
        // payload. Its cost must precede a stale-key Conflict as well.
        reader.work(2 * field.storage_length())?;
        let mut work = KeyWork {
            budget: reader.budget,
            cancelled,
        };
        let expected = profile.row_key(
            &Cell {
                column,
                value: change.expected,
            },
            &mut work,
        )?;
        if !profile.compare(&entry.value.key, &expected).is_eq() {
            return Err(EditError::Conflict);
        }
        check(work.budget, cancelled)?;
        work.budget
            .charge(Resource::Steps, binary::MAX_ENTRY_BYTES as u64)
            .map_err(InspectError::from)?;
        let mut encoded = [0; binary::MAX_ENTRY_BYTES];
        let width = binary::encode_component(
            change.value,
            field.storage_length(),
            key.descending(),
            &mut encoded,
        )?;
        check(work.budget, cancelled)?;
        work.budget
            .charge(Resource::Steps, width as u64)
            .map_err(InspectError::from)?;
        entry.value.key = profile.decode(&encoded[..width], &mut work)?.0;
        matched += 1;
    }
    if matched != changes.len() {
        return Err(EditError::Preservation);
    }
    if changes.is_empty() {
        return Ok(Vec::new());
    }
    let levels = root.entries.len().checked_ilog2().unwrap_or(0) as usize + 1;
    reader.work(root.entries.len() * levels * profile.max_key_bytes())?;
    root.entries
        .sort_unstable_by(|left, right| left.value.compare_with(&right.value, &profile));
    let after = repack::retained_page::<_, { binary::MAX_ENTRY_BYTES }>(
        before,
        &root.entries,
        &profile,
        reader.budget,
        cancelled,
    )?;
    let mut result = Vec::new();
    repack::patches(
        before,
        &after,
        index.root_page(),
        &mut result,
        reader.budget,
        cancelled,
    )?;
    check(reader.budget, cancelled)?;
    Ok(result)
}
