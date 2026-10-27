//! Exact-slot replacements use genuine owned encoded keys for index splitting.
use super::super::replacement::{self, ReplacementProfile, SlotKeyChanges, SlotKeyUpdate};
use super::{Plan, edit_prepared, nonroot, seal};
use crate::{
    EditError, IndexCheck, InspectError, NativeImage, NativeIndex, TableSchema, WriteBlocker,
    format::{
        data_type::{SHORT_BINARY, SHORT_TEXT},
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
    table::{
        indexes::{IndexDataPatch, binary, text},
        mutation::append::State,
    },
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::table) fn qualify(
    schema: &TableSchema,
    indexes: &[IndexCheck],
    column: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    replacement::qualify(schema, indexes, column, budget, cancelled)?;

    for checked in indexes {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, checked.index().columns().len() as u64)
            .map_err(InspectError::from)?;
        let index = checked.index();
        if index
            .columns()
            .iter()
            .any(|key| key.number() == schema.columns[column].number())
            && index.columns().len() != 1
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }

    Ok(())
}

pub(in crate::table) fn patches(
    image: &NativeImage,
    schema: &TableSchema,
    indexes: &[IndexCheck],
    column: usize,
    updates: &[SlotKeyUpdate<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Plan, EditError> {
    continue_patches(
        image,
        schema,
        indexes,
        &SlotKeyChanges { column, updates },
        State::new(image, schema.definition),
        budget,
        cancelled,
    )
}

pub(in crate::table) fn continue_patches(
    image: &NativeImage,
    schema: &TableSchema,
    indexes: &[IndexCheck],
    changes: &SlotKeyChanges<'_>,
    append: State,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Plan, EditError> {
    let column = changes.column;
    let updates = changes.updates;
    qualify(schema, indexes, column, budget, cancelled)?;

    if append.original_length != image.as_bytes().len()
        || append.owner != schema.definition
        || append.length < append.original_length
        || !append.length.is_multiple_of(PAGE_BYTES)
    {
        return Err(EditError::Preservation);
    }
    append.preview(0)?;

    let mut plan = Plan::with_append(append);
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

        let metadata = &schema.columns[column];
        let descending = index.columns()[0].descending();
        match metadata.native_type() {
            SHORT_TEXT => edit::<_, { text::MAX_ENTRY_BYTES }>(
                Operation {
                    image,
                    index,
                    column,
                    profile: text::TextProfile::new(column, metadata, descending)?,
                    updates,
                },
                &mut reader,
                &mut patches,
                &mut plan,
            )?,
            SHORT_BINARY => edit::<_, { binary::MAX_ENTRY_BYTES }>(
                Operation {
                    image,
                    index,
                    column,
                    profile: binary::BinaryProfile {
                        column,
                        descending,
                        maximum: metadata.storage_length(),
                        selection: binary::BinaryIndexSelection::All,
                    },
                    updates,
                },
                &mut reader,
                &mut patches,
                &mut plan,
            )?,
            _ => return Err(EditError::Unsupported(WriteBlocker::IndexedColumn)),
        }
    }

    plan.patches = patches;
    seal(image, indexes, &mut plan, budget, cancelled)?;

    Ok(plan)
}

struct Operation<'a, P> {
    image: &'a NativeImage,
    index: &'a NativeIndex,
    column: usize,
    profile: P,
    updates: &'a [SlotKeyUpdate<'a>],
}

fn edit<P: ReplacementProfile, const N: usize>(
    operation: Operation<'_, P>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    patches: &mut Vec<IndexDataPatch>,
    plan: &mut Plan,
) -> Result<(), EditError> {
    let prepared = replacement::keys::prepare::<P, N>(
        operation.image,
        reader,
        plan.append.owner,
        operation.index,
        operation.profile,
        operation.updates,
    )?;
    if operation.updates.is_empty() {
        return Ok(());
    }

    edit_prepared(
        &nonroot::Operation::<_, N> {
            image: operation.image,
            index: operation.index,
            profile: &prepared.profile,
            keys: &prepared.entries,
        },
        &prepared.tree,
        operation.column,
        patches,
        plan,
        reader,
    )
}
