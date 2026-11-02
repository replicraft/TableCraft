//! Existing-page typed deletions below normal admission and journal packaging.
mod binary;
mod tree;
use super::{
    IndexDataPatch, NumericKey, encoding, leaf_removal, profile::SingleProfile, repack,
    retirement::RetiredPage,
};
use crate::{
    EditError, IndexCheckStatus, InspectError, NativeImage, TableCheck, WriteBlocker,
    format::{page::PAGE_BYTES, reader::Reader},
    header::check,
    table::indexes::page,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

/// Working physical changes and original-derived retirement facts. The normal
/// caller still owns complete Source, writable-map qualification and proof.
pub(in crate::table) struct Deletions {
    pub patches: Vec<IndexDataPatch>,
    pub retired: Vec<RetiredPage>,
}

/// The caller holds complete ordinary Source, checked typed declarations,
/// catalog-wide exclusive INDEX ownership and live-LVAL exclusion. Only the
/// existing Int32 path can redistribute an already owned NODE topology.
pub(in crate::table) fn patches(
    image: &NativeImage,
    checked: &TableCheck,
    references: &[u32],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Deletions, EditError> {
    let _span = tracing::debug_span!("native.ordinary.delete.index_patches").entered();
    let mut result = Deletions {
        patches: Vec::new(),
        retired: Vec::new(),
    };
    let schema = checked.schema();
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
        if checked_index.status() != IndexCheckStatus::Checked {
            return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
        }
        if checked.indexes()[..position]
            .iter()
            .any(|prior| prior.index().data_number() == index.data_number())
        {
            continue;
        }
        let [key] = index.columns() else {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        };
        reader.work(schema.columns().len())?;
        let column = schema
            .columns()
            .iter()
            .position(|field| field.number() == key.number())
            .ok_or(InspectError::Malformed)?;
        if schema.columns()[column].scalar_type() == Some(ScalarType::Binary) {
            binary::patches(
                image,
                &mut reader,
                index,
                schema,
                column,
                references,
                &mut result.patches,
            )?;
            continue;
        }
        let kind = match schema.columns()[column].scalar_type() {
            Some(kind @ (ScalarType::Int32 | ScalarType::Int64 | ScalarType::Currency)) => kind,
            _ => return Err(EditError::Unsupported(WriteBlocker::IndexedColumn)),
        };
        let profile = SingleProfile {
            column,
            kind: NumericKey::new(kind)?,
            descending: key.descending(),
        };
        let mut root = page::read_profile::<_, _, { encoding::MAX_ENTRY_BYTES }>(
            &mut reader,
            index.root_page(),
            schema.definition_page(),
            &profile,
        )?;
        let start = (index.root_page() as usize)
            .checked_mul(PAGE_BYTES)
            .ok_or(InspectError::StructureLimit)?;
        let end = start
            .checked_add(PAGE_BYTES)
            .ok_or(InspectError::StructureLimit)?;
        let original = image
            .as_bytes()
            .get(start..end)
            .ok_or(InspectError::Truncated)?;
        if !root.leaf {
            drop(root);
            tree::patches(
                image,
                &mut reader,
                index,
                schema.definition_page(),
                references,
                &profile,
                &mut result,
            )?;
            continue;
        }
        leaf_removal::require_root(&root, original)?;
        leaf_removal::remove(&mut root.entries, references, reader.budget, cancelled)?;
        if references.is_empty() {
            continue;
        }
        let after = repack::page::<_, { encoding::MAX_ENTRY_BYTES }>(
            original,
            &root.entries,
            &profile,
            reader.budget,
            cancelled,
        )?;
        repack::patches(
            original,
            &after,
            index.root_page(),
            &mut result.patches,
            reader.budget,
            cancelled,
        )?;
    }
    check(reader.budget, cancelled)?;
    Ok(result)
}
