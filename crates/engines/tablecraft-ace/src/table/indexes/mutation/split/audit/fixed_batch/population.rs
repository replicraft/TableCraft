//! Read complete original logical membership, including physically omitted rows.
use super::super::super::super::{profile::WriteProfile, tree::Tree};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, TableSchema,
    format::{reader::Reader, row},
    table::{
        indexes::{
            key::{KeyRow, KeyWork},
            mutation::IndexKeyUpdate,
            page::KeyRef,
        },
        page::{self, TableSnapshotLimits},
        snapshot,
    },
};
use std::io::Cursor;

pub(super) struct Population<K> {
    pub tree: Tree,
    pub keys: Vec<KeyRef<K>>,
    pub history_delta: usize,
}

struct Projected<'a> {
    positions: &'a [usize],
    values: &'a [tablecraft_core::values::Value],
}

impl KeyRow for Projected<'_> {
    fn get(&self, column: usize) -> Option<&tablecraft_core::values::Value> {
        self.positions
            .iter()
            .position(|position| *position == column)
            .and_then(|position| self.values.get(position))
    }
}

pub(super) fn derive<P: WriteProfile, const N: usize>(
    image: &NativeImage,
    schema: &TableSchema,
    index: &NativeIndex,
    updates: &[IndexKeyUpdate],
    profile: &P,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Population<P::Key>, EditError> {
    let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
    let mut raw_keys = Vec::new();
    let tree = Tree::read::<P, N>(
        image,
        reader,
        schema.definition_page(),
        index.root_page(),
        profile,
        0,
        &mut raw_keys,
    )?;
    let mut reached = Vec::new();
    tree_pages(&tree, &mut reached, reader)?;
    let levels = reached.len().checked_ilog2().unwrap_or(0) as usize + 1;
    reader.work(reached.len().saturating_mul(levels))?;
    reached.sort_unstable();
    if reached.windows(2).any(|pair| pair[0] == pair[1])
        || reached != crate::format::allocation::pages(reader, index.usage_map())?
    {
        return Err(EditError::Preservation);
    }
    let mut names = Vec::new();
    for component in index.columns() {
        reader.tick()?;
        reader.work(schema.columns().len())?;
        let column = schema
            .columns()
            .iter()
            .find(|column| column.number() == component.number())
            .ok_or(InspectError::Malformed)?;
        super::push(&mut names, column.name(), reader)?;
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
    let mut before = Vec::new();
    let mut after = Vec::new();
    let mut transitions = Vec::new();
    let mut matched = 0usize;
    let mut previous = None;
    for (reference, values) in rows {
        reader.tick()?;
        if previous.is_some_and(|prior| prior >= reference) {
            return Err(EditError::Preservation);
        }
        previous = Some(reference);
        let reference = row::reference(reference.page_number(), reference.slot_number());
        let original = profile.row_key(
            &Projected {
                positions: &positions,
                values: &values,
            },
            &mut KeyWork {
                budget: reader.budget,
                cancelled: reader.cancelled,
            },
        )?;
        let mut final_key = original;
        reader.work(updates.len().saturating_mul(profile.components()))?;
        for update in updates
            .iter()
            .filter(|update| update.reference == reference && profile.includes(update.column))
        {
            profile.update(&mut final_key, update.column, update.expected, update.value)?;
            matched = matched.checked_add(1).ok_or(InspectError::StructureLimit)?;
        }
        let (old_all_null, _) = profile.constraint_nulls(&original)?;
        let (all_null, any_null) = profile.constraint_nulls(&final_key)?;
        if constraints.required() && any_null {
            return Err(
                InspectError::IndexConstraint(crate::IndexConstraintViolation::NullKey).into(),
            );
        }
        if !constraints.ignore_nulls() || !old_all_null {
            super::push(
                &mut before,
                KeyRef {
                    key: original,
                    row: reference,
                },
                reader,
            )?;
        }
        if !constraints.ignore_nulls() || !all_null {
            super::push(
                &mut after,
                KeyRef {
                    key: final_key,
                    row: reference,
                },
                reader,
            )?;
        }
        super::push(&mut transitions, (original, final_key), reader)?;
    }
    reader.work(updates.len())?;
    if matched
        != updates
            .iter()
            .filter(|update| profile.includes(update.column))
            .count()
    {
        return Err(EditError::Preservation);
    }
    sort(&mut before, profile, reader)?;
    sort(&mut after, profile, reader)?;
    if raw_keys.len() != before.len() {
        return Err(EditError::Preservation);
    }
    for (actual, expected) in raw_keys.iter().zip(&before) {
        reader.tick()?;
        if actual.compare_with(expected, profile).is_ne() {
            return Err(EditError::Preservation);
        }
    }
    if constraints.unique() {
        for pair in after.windows(2) {
            reader.tick()?;
            if !profile.constraint_nulls(&pair[0].key)?.0
                && profile.compare(&pair[0].key, &pair[1].key).is_eq()
            {
                return Err(InspectError::IndexConstraint(
                    crate::IndexConstraintViolation::DuplicateKey,
                )
                .into());
            }
        }
    }
    let history_delta = if constraints.ignore_nulls() {
        super::history::delta(&before, &transitions, profile, reader)?
    } else {
        0
    };
    Ok(Population {
        tree,
        keys: after,
        history_delta,
    })
}

fn sort<P: WriteProfile>(
    keys: &mut [KeyRef<P::Key>],
    profile: &P,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let levels = keys.len().checked_ilog2().unwrap_or(0) as usize + 1;
    reader.work(
        keys.len()
            .saturating_mul(levels)
            .saturating_mul(profile.max_key_bytes()),
    )?;
    keys.sort_unstable_by(|left, right| left.compare_with(right, profile));
    for pair in keys.windows(2) {
        reader.tick()?;
        if pair[0].compare_with(&pair[1], profile).is_ge() {
            return Err(EditError::Preservation);
        }
    }
    Ok(())
}

fn tree_pages(
    tree: &Tree,
    pages: &mut Vec<u32>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.tick()?;
    super::push(pages, tree.number(), reader)?;
    for child in tree.children() {
        tree_pages(child, pages, reader)?;
    }
    Ok(())
}
