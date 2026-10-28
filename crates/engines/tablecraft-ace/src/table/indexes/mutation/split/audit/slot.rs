//! Reprepare encoded replacement keys from immutable slot request authority.
use super::super::super::{
    replacement::{self, SlotKeyChanges},
    row_key::RowKeyProfile,
};
use super::super::Plan;
use super::retained;
use crate::{
    EditError, NativeImage, NativeIndex, TableSchema, WriteBlocker,
    format::{
        data_type::{SHORT_BINARY, SHORT_TEXT},
        reader::Reader,
    },
    table::indexes::{binary, text},
};
use std::io::Cursor;

pub(super) struct Operation<'a> {
    pub image: &'a NativeImage,
    pub schema: &'a TableSchema,
    pub index: &'a NativeIndex,
    pub changes: &'a SlotKeyChanges<'a>,
}

pub(super) fn prove(
    operation: &Operation<'_>,
    plan: &Plan,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
    progress: &mut retained::Progress<'_>,
) -> Result<retained::Disposition, EditError> {
    let column = operation.changes.column;
    let metadata = &operation.schema.columns[column];
    let index = operation.index;
    if index.columns().len() != 1 || index.columns()[0].number() != metadata.number() {
        return Err(EditError::Preservation);
    }
    let descending = index.columns()[0].descending();

    match metadata.native_type() {
        SHORT_TEXT => prepared::<_, { text::MAX_ENTRY_BYTES }>(
            operation,
            text::TextProfile::new(column, metadata, descending)?,
            plan,
            actual,
            progress,
        ),
        SHORT_BINARY => prepared::<_, { binary::MAX_ENTRY_BYTES }>(
            operation,
            binary::BinaryProfile {
                column,
                descending,
                maximum: metadata.storage_length(),
                selection: binary::BinaryIndexSelection::All,
            },
            plan,
            actual,
            progress,
        ),
        _ => Err(EditError::Unsupported(WriteBlocker::IndexedColumn)),
    }
}

fn prepared<P: RowKeyProfile, const N: usize>(
    operation: &Operation<'_>,
    original: P,
    plan: &Plan,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
    progress: &mut retained::Progress<'_>,
) -> Result<retained::Disposition, EditError> {
    let mut source = operation.image.reader();
    let prepared = replacement::keys::prepare::<P, N>(
        operation.image,
        &mut Reader {
            source: &mut source,
            header: operation.image.catalog().header(),
            budget: actual.budget,
            cancelled: actual.cancelled,
            reads: 0,
        },
        operation.schema.definition,
        operation.index,
        original,
        operation.changes.updates,
    )?;

    retained::prove::<_, N>(
        &retained::Operation {
            image: operation.image,
            index: operation.index,
            owner: operation.schema.definition,
            column: operation.changes.column,
            profile: &prepared.profile,
            tree: &prepared.tree,
            keys: &prepared.entries,
        },
        plan,
        actual,
        progress,
    )
}
