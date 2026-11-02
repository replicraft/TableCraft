//! Existing-only typed numeric UPDATE below actual normal Source admission.
use super::{IndexDataPatch, IndexKeyUpdate, NumericKey, normal_fitting, profile::SingleProfile};
use crate::{
    EditError, NativeImage, TableCheck, WriteBlocker,
    format::{
        data_type::{CURRENCY, INT32, INT64},
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
    table::indexes::{
        header::IndexHeader,
        page::{LEAF, NODE},
    },
};

use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

/// Caller owns fresh checked source identity, catalog-wide INDEX protection and
/// live LVAL exclusion. This returns physical patches, never a durable plan.
pub(in crate::table) fn patches(
    image: &NativeImage,
    checked: &TableCheck,
    updates: &[IndexKeyUpdate],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<IndexDataPatch>, EditError> {
    patches_with(
        image,
        checked,
        updates,
        Profile::RootLeaves,
        budget,
        cancelled,
    )
}

pub(in crate::table) fn patches_tree(
    image: &NativeImage,
    checked: &TableCheck,
    updates: &[IndexKeyUpdate],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<IndexDataPatch>, EditError> {
    patches_with(
        image,
        checked,
        updates,
        Profile::RetainedTrees,
        budget,
        cancelled,
    )
}

#[derive(Clone, Copy)]
enum Profile {
    RootLeaves,
    RetainedTrees,
}

fn patches_with(
    image: &NativeImage,
    checked: &TableCheck,
    updates: &[IndexKeyUpdate],
    selected_profile: Profile,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<IndexDataPatch>, EditError> {
    let schema = checked.schema();
    let mut patches = Vec::new();
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    for (position, selected) in checked.indexes().iter().enumerate() {
        reader.tick()?;
        reader.work(position)?;
        let index = selected.index();
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
        let (column, field) = schema
            .columns()
            .iter()
            .enumerate()
            .find(|(_, field)| field.number() == key.number())
            .ok_or(EditError::Preservation)?;
        let kind = match (field.native_type(), field.scalar_type()) {
            (INT32, Some(ScalarType::Int32)) => ScalarType::Int32,
            (INT64, Some(ScalarType::Int64)) => ScalarType::Int64,
            (CURRENCY, Some(ScalarType::Currency)) => ScalarType::Currency,
            _ => return Err(EditError::Unsupported(WriteBlocker::IndexedColumn)),
        };
        let start = index.root_page() as usize * PAGE_BYTES;
        let raw = image
            .as_bytes()
            .get(start..start + PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        reader.work(size_of::<IndexHeader>())?;
        let header = IndexHeader::read(raw)?;
        let accepted = match selected_profile {
            Profile::RootLeaves => {
                header.kind == LEAF
                    && header.qualified_for_update(0)
                    && header.prefix.get() == 0
                    && header.previous.get() == 0
                    && header.next.get() == 0
                    && header.tail.get() == 0
            }
            Profile::RetainedTrees => matches!(header.kind, LEAF | NODE),
        };
        if !accepted {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        // The retained caller has checked every actual owned descendant. The
        // real preparation below rechecks update stamp/height while reading
        // affected trees, then rewrites their original fixed leaf counts.
        reader.work(updates.len())?;
        if !updates.iter().any(|update| update.column == column) {
            continue;
        }
        let profile = SingleProfile {
            column,
            kind: NumericKey::new(kind)?,
            descending: key.descending(),
        };
        normal_fitting::rewrite::<_, { super::encoding::MAX_ENTRY_BYTES }>(
            image,
            &mut reader,
            index,
            schema,
            updates,
            &profile,
            &mut patches,
        )?;
    }
    check(reader.budget, cancelled)?;
    Ok(patches)
}
