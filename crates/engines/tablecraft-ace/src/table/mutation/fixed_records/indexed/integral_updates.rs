//! Closed wider UPDATE facts; action read coverage alone never grants a target.
use super::{CheckedIndexes, IndexTargets};
use crate::{
    EditError, IndexCheckStatus, InspectError, NativeImage, TableCheck, TableSchema, WriteBlocker,
    format::{
        data_type::{CURRENCY, INT32, INT64},
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        page::PAGE_BYTES,
    },
    header::check,
    table::{
        indexes::{
            header::IndexHeader,
            mutation::normal_index_audit::Ownership,
            page::{LEAF, NODE},
        },
        mutation::{lval_ownership::Inventory, preserved_source::Source},
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Profile {
    RootLeaves,
    RetainedTrees,
}

pub(in crate::table::mutation) struct IntegralUpdates<'image> {
    base: CheckedIndexes<'image>,
    profile: Profile,
    pub(in crate::table::mutation) ownership: Ownership,
}

pub(in crate::table::mutation) fn width(
    native: u8,
    kind: Option<ScalarType>,
) -> Result<usize, EditError> {
    match (native, kind) {
        (INT32, Some(ScalarType::Int32)) => Ok(size_of::<i32>()),
        (INT64, Some(ScalarType::Int64)) | (CURRENCY, Some(ScalarType::Currency)) => {
            Ok(size_of::<i64>())
        }
        _ => Err(EditError::Unsupported(WriteBlocker::IndexedColumn)),
    }
}

impl<'image> IntegralUpdates<'image> {
    pub(in crate::table::mutation) fn capture(
        image: &'image NativeImage,
        source: &Source,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_with(image, source, Profile::RootLeaves, budget, cancelled)
    }

    pub(in crate::table::mutation) fn capture_tree(
        image: &'image NativeImage,
        source: &Source,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        Self::capture_with(image, source, Profile::RetainedTrees, budget, cancelled)
    }

    fn capture_with(
        image: &'image NativeImage,
        source: &Source,
        profile: Profile,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        let base = CheckedIndexes::capture_actions(
            image,
            source.table.schema().name(),
            budget,
            cancelled,
        )?;
        base.require_source(image, source.table.schema(), budget, cancelled)?;
        let checked = base.checked();
        let schema = checked.schema();
        if checked.rows() != source.records.len()
            || schema.scalar_columns_with_budget(budget)? != source.table.columns()
            || checked.indexes().len() != source.indexes.indexes().len()
        {
            return Err(EditError::Preservation);
        }
        for (position, field) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            let size = width(field.native_type(), field.scalar_type())?;
            if field.number() != position
                || !field.fixed()
                || field.storage_length() != size
                || field.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
                || field.extended_flags() != 0
                || field.complex_type_id().is_some()
            {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
        }
        let mut wide = false;
        let mut has_node = false;
        budget
            .charge_allocation(checked.indexes().len() as u64, size_of::<u32>() as u64)
            .map_err(InspectError::from)?;
        let mut pages = Vec::new();
        pages
            .try_reserve_exact(checked.indexes().len())
            .map_err(|_| InspectError::Allocation)?;
        for (position, selected) in checked.indexes().iter().enumerate() {
            check(budget, cancelled)?;
            let index = selected.index();
            let [key] = index.columns() else {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            };
            let flags = index.constraints().ok_or(InspectError::Unsupported)?;
            budget
                .charge(Resource::Steps, schema.columns().len() as u64)
                .map_err(InspectError::from)?;
            let field = schema
                .columns()
                .iter()
                .find(|field| field.number() == key.number())
                .ok_or(EditError::Preservation)?;
            wide |= matches!(field.native_type(), INT64 | CURRENCY);
            if selected.status() != IndexCheckStatus::Checked
                || index.native_type() > 1
                || index.foreign_reference().is_some()
                || flags.ignore_nulls()
            {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            let start = index.root_page() as usize * PAGE_BYTES;
            let raw = image
                .as_bytes()
                .get(start..start + PAGE_BYTES)
                .ok_or(EditError::Preservation)?;
            budget
                .charge(Resource::Steps, size_of::<IndexHeader>() as u64)
                .map_err(InspectError::from)?;
            let header = IndexHeader::read(raw)?;
            has_node |= header.kind == NODE;
            let accepted = match profile {
                Profile::RootLeaves => {
                    header.kind == LEAF
                        && header.qualified_for_update(0)
                        && header.prefix.get() == 0
                        && header.previous.get() == 0
                        && header.next.get() == 0
                        && header.tail.get() == 0
                }
                Profile::RetainedTrees => {
                    matches!(header.kind, LEAF | NODE)
                        && header.previous.get() == 0
                        && header.next.get() == 0
                }
            };
            if !accepted {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            budget
                .charge(Resource::Steps, position as u64)
                .map_err(InspectError::from)?;
            if !checked.indexes()[..position]
                .iter()
                .any(|prior| prior.index().data_number() == index.data_number())
            {
                pages.push(index.root_page());
            }
        }
        if !wide || (profile == Profile::RetainedTrees && !has_node) {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        let ownership = match profile {
            Profile::RootLeaves => Ownership::capture_actions(image, checked, budget, cancelled)?,
            Profile::RetainedTrees => {
                Ownership::capture_integral_updates(image, checked, budget, cancelled)?
            }
        };
        for selected in checked.indexes() {
            ownership.require(image, schema, selected.index(), budget, cancelled)?;
        }
        let inventory = Inventory::capture(image, budget, cancelled)?;
        if profile == Profile::RootLeaves {
            inventory.require_no_long_value_data_overlap(&pages, budget, cancelled)?;
        } else {
            // Every descendant of every actual physical tree is mutable index
            // storage. Roots alone cannot exclude readonly live LVAL overlap.
            for (position, selected) in checked.indexes().iter().enumerate() {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, position as u64)
                    .map_err(InspectError::from)?;
                if checked.indexes()[..position]
                    .iter()
                    .any(|prior| prior.index().data_number() == selected.index().data_number())
                {
                    continue;
                }
                let tree = ownership.require(image, schema, selected.index(), budget, cancelled)?;
                inventory.require_no_long_value_data_overlap(tree.pages(), budget, cancelled)?;
            }
        }
        check(budget, cancelled)?;
        Ok(Self {
            base,
            profile,
            ownership,
        })
    }

    pub(in crate::table::mutation) fn retained_tree(&self) -> bool {
        self.profile == Profile::RetainedTrees
    }

    pub(in crate::table::mutation) fn checked(&self) -> &TableCheck {
        self.base.checked()
    }
}

impl IndexTargets for IntegralUpdates<'_> {
    fn require_source(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.base.require_source(image, schema, budget, cancelled)
    }
    fn require_target(
        &self,
        schema: &TableSchema,
        position: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        self.base
            .require_source(self.base.image, schema, budget, cancelled)?;
        let field = schema
            .columns()
            .get(position)
            .ok_or(EditError::Preservation)?;
        let size = width(field.native_type(), field.scalar_type())?;
        if field.number() != position
            || !field.fixed()
            || field.storage_length() != size
            || field.extended_flags() != 0
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        Ok(())
    }
}
