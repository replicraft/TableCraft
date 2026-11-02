//! Preselected fitting UPDATE serialization for the normal ACE Int32 profile.
//! This does not replace ordinary Jet4 index_patches or retry a failed encoding.
use super::{
    IndexDataPatch, IndexKeyUpdate, NumericKey,
    composite::TupleProfile,
    encoding::{self, Mode},
    normal_index_audit::Ownership,
    plan::keys,
    profile::{SingleProfile, WriteProfile},
    tree::Rewrite,
};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, TableCheck, TableSchema, WriteBlocker,
    format::{data_type::INT32, reader::Reader},
    header::check,
    table::indexes::composite,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

pub(in crate::table) fn patches(
    image: &NativeImage,
    checked: &TableCheck,
    updates: &[IndexKeyUpdate],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<IndexDataPatch>, EditError> {
    let _span = tracing::debug_span!("native.before_change.index_fitting").entered();
    check(budget, cancelled)?;
    let schema = checked.schema();
    // Qualify every original physical declaration even when updates are empty.
    // The caller separately qualifies required programs or complete ordinary
    // content. These physical index facts supply neither kind of admission.
    let ownership = Ownership::capture(image, checked, budget, cancelled)?;
    let mut patches = Vec::new();
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    for (position, checked_index) in checked.indexes().iter().enumerate() {
        reader.tick()?;
        reader.work(position)?;
        let index = checked_index.index();
        if checked.indexes()[..position]
            .iter()
            .any(|prior| prior.index().data_number() == index.data_number())
        {
            continue;
        }
        ownership.require(image, schema, index, reader.budget, cancelled)?;
        let mut affected = false;
        for key in index.columns() {
            reader.tick()?;
            reader.work(schema.columns().len())?;
            let column = schema
                .columns()
                .iter()
                .position(|column| column.number() == key.number())
                .ok_or(InspectError::Malformed)?;
            let metadata = &schema.columns()[column];
            if metadata.native_type() != INT32
                || metadata.scalar_type() != Some(ScalarType::Int32)
                || !metadata.fixed()
                || metadata.storage_length() != size_of::<i32>()
            {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            reader.work(updates.len())?;
            affected |= updates.iter().any(|update| update.column == column);
        }
        if !affected {
            continue;
        }
        if index.columns().len() == 1 {
            let number = index.columns()[0].number();
            reader.work(schema.columns().len())?;
            let column = schema
                .columns()
                .iter()
                .position(|column| column.number() == number)
                .ok_or(InspectError::Malformed)?;
            let profile = SingleProfile {
                column,
                kind: NumericKey::new(ScalarType::Int32)?,
                descending: index.columns()[0].descending(),
            };
            rewrite::<_, { encoding::MAX_ENTRY_BYTES }>(
                image,
                &mut reader,
                index,
                schema,
                updates,
                &profile,
                &mut patches,
            )?;
        } else {
            let profile = TupleProfile::<i64>::new(index, schema, reader.budget, cancelled)?;
            rewrite::<_, { composite::MAX_ENTRY_BYTES }>(
                image,
                &mut reader,
                index,
                schema,
                updates,
                &profile,
                &mut patches,
            )?;
        }
    }
    check(reader.budget, cancelled)?;
    Ok(patches)
}

pub(super) fn rewrite<P: WriteProfile, const N: usize>(
    image: &NativeImage,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    index: &NativeIndex,
    schema: &TableSchema,
    updates: &[IndexKeyUpdate],
    profile: &P,
    patches: &mut Vec<IndexDataPatch>,
) -> Result<(), EditError> {
    // Select the fitting layout before preparation/serialization. No Fixed
    // encoder is attempted, and no encoding failure selects another profile.
    let selected = Mode::Repack;
    let prepared = keys::prepare::<_, N>(
        image,
        reader,
        index,
        schema.definition_page(),
        updates,
        profile,
    )?;
    let mut remaining = prepared.entries.as_slice();
    prepared.tree.rewrite::<_, N>(
        image,
        &mut remaining,
        &Rewrite {
            profile,
            mode: selected,
        },
        patches,
        reader.budget,
        reader.cancelled,
    )?;
    if !remaining.is_empty() {
        return Err(EditError::Preservation);
    }
    Ok(())
}
