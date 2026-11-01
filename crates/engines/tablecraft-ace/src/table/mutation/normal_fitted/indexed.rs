//! Original named Binary targets select fitting indexed UPDATE before staging.
pub(super) mod proof;

use super::{binding, stage};
use crate::{
    EditError, FittedColumnUpdates, InspectError, NativeEditPlan, NativeImage, WriteBlocker,
    format::{data_type::SHORT_BINARY, page::PAGE_BYTES},
    header::check,
    inspect_table_indexes, inspect_table_schema,
    table::{
        indexes::mutation::{
            IndexDataPatch,
            normal_binary_update::{self, Change},
            normal_index_audit::Ownership,
        },
        mutation::{
            fixed_records::{self, indexed::CheckedIndexes},
            lval_ownership::Inventory,
            normal_actions, ordinary_content,
            preserved_source::{Source, validation},
            record_serialization,
        },
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const LEAF_KIND: u8 = 4;
const PREFIX_AT: usize = 24;
const PREFIX_BYTES: usize = size_of::<u16>();

pub(in crate::table::mutation) fn selected(
    image: &NativeImage,
    table: &str,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    let indexes = inspect_table_indexes(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    for group in groups {
        let FittedColumnUpdates::Variable { column, .. } = group else {
            continue;
        };
        for field in indexes.schema().columns() {
            check(budget, cancelled)?;
            fixed_records::name_work(field.name(), column, budget)?;
            if field.native_type() != SHORT_BINARY || !field.name().eq_ignore_ascii_case(column) {
                continue;
            }
            for index in indexes.indexes() {
                for key in index.columns() {
                    check(budget, cancelled)?;
                    if key.number() == field.number() {
                        return Ok(true);
                    }
                }
            }
        }
    }
    Ok(false)
}

pub(super) fn capability_selected(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    let indexes = inspect_table_indexes(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    for field in indexes.schema().columns() {
        check(budget, cancelled)?;
        if field.native_type() == SHORT_BINARY && super::variable_shape(field) {
            for index in indexes.indexes() {
                for key in index.columns() {
                    check(budget, cancelled)?;
                    if key.number() == field.number() {
                        return Ok(true);
                    }
                }
            }
        }
    }
    Ok(false)
}

pub(super) fn capture(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    let schema = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    ordinary_content::definition::validate_binary_updates(image, &schema, budget, cancelled)?;
    // The complete native17 absence, properties, dependencies, relationships,
    // protection and retained-index Source checks are freshly repeated here.
    normal_actions::capture(image, table, budget, cancelled)
}

pub(super) fn qualify<'image>(
    image: &'image NativeImage,
    source: &Source,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<CheckedIndexes<'image>, EditError> {
    let facts = CheckedIndexes::capture_actions(image, table, budget, cancelled)?;
    facts.require_source(image, source.table.schema(), budget, cancelled)?;
    if facts.checked().rows() != source.records.len() {
        return Err(EditError::Preservation);
    }
    let ownership = Ownership::capture_actions(image, facts.checked(), budget, cancelled)?;
    let live = Inventory::capture(image, budget, cancelled)?;
    for index in facts.checked().indexes() {
        check(budget, cancelled)?;
        let index = index.index();
        let [key] = index.columns() else {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        };
        let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
        if index.native_type() > 1
            || index.foreign_reference().is_some()
            || constraints.ignore_nulls()
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        let tree = ownership.require(image, source.table.schema(), index, budget, cancelled)?;
        live.require_no_long_value_data_overlap(tree.pages(), budget, cancelled)?;
        let field = source
            .table
            .schema()
            .columns()
            .get(key.number())
            .ok_or(EditError::Preservation)?;
        if field.native_type() == SHORT_BINARY {
            let base = index.root_page() as usize * PAGE_BYTES;
            let page = image
                .as_bytes()
                .get(base..base + PAGE_BYTES)
                .ok_or(EditError::Preservation)?;
            if tree.pages() != [index.root_page()]
                || page[0] != LEAF_KIND
                || page[PREFIX_AT..PREFIX_AT + PREFIX_BYTES] != [0; PREFIX_BYTES]
            {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
        }
    }
    Ok(facts)
}

pub(super) fn check_table(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let source = capture(image, table, budget, cancelled)?;
    let facts = qualify(image, &source, table, budget, cancelled)?;
    for field in source.table.schema().columns() {
        check(budget, cancelled)?;
        if field.native_type() != SHORT_BINARY || !super::variable_shape(field) {
            continue;
        }
        let groups = [FittedColumnUpdates::Variable {
            column: field.name(),
            updates: &[],
        }];
        if selected(image, table, &groups, budget, cancelled)? {
            binding::all_indexed_binary(&source, &groups, budget, cancelled)?;
            drop(facts);
            return Ok(());
        }
    }
    Err(EditError::Unsupported(WriteBlocker::Column))
}

pub(super) fn index_patches(
    image: &NativeImage,
    source: &Source,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<IndexDataPatch>, EditError> {
    let table = source.table.schema().name();
    let facts = qualify(image, source, table, budget, cancelled)?;
    let bound = binding::all_indexed_binary(source, groups, budget, cancelled)?;
    let mut patches = Vec::new();
    for (position, checked) in facts.checked().indexes().iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, position as u64)
            .map_err(InspectError::from)?;
        let index = checked.index();
        if facts.checked().indexes()[..position]
            .iter()
            .any(|prior| prior.index().data_number() == index.data_number())
        {
            continue;
        }
        let [key] = index.columns() else {
            return Err(EditError::Preservation);
        };
        let mut changes =
            binding::vector(groups.iter().map(FittedColumnUpdates::len).sum(), budget)?;
        for (group, target) in groups.iter().zip(&bound.targets) {
            if target.column() != key.number() {
                continue;
            }
            let FittedColumnUpdates::Variable { updates, .. } = group else {
                // Fixed indexed targets remain with their original TCNI4 service.
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            };
            for update in *updates {
                check(budget, cancelled)?;
                let actual = source
                    .table
                    .rows()
                    .get(update.row())
                    .and_then(|row| row.get(target.column()))
                    .ok_or(EditError::Conflict)?;
                if !binding::equal(actual, update.expected(), budget, cancelled)? {
                    return Err(EditError::Conflict);
                }
                if !binding::equal(update.expected(), update.value(), budget, cancelled)? {
                    changes.push(Change {
                        reference: source.records[update.row()].reference,
                        expected: update.expected(),
                        value: update.value(),
                    });
                }
            }
        }
        if source.table.schema().columns()[key.number()].native_type() != SHORT_BINARY {
            continue;
        }
        let next = normal_binary_update::patches(
            image,
            source.table.schema(),
            index,
            &changes,
            budget,
            cancelled,
        )?;
        budget
            .charge_allocation(next.len() as u64, size_of::<IndexDataPatch>() as u64)
            .map_err(InspectError::from)?;
        patches
            .try_reserve_exact(next.len())
            .map_err(|_| InspectError::Allocation)?;
        patches.extend(next);
    }
    Ok(patches)
}

pub(in crate::table::mutation) fn preflight(
    image: &NativeImage,
    table: &str,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let source = capture(image, table, budget, cancelled)?;
    stage::prepare_indexed_binary(image, &source, groups, budget, cancelled)?;
    index_patches(image, &source, groups, budget, cancelled)?;
    Ok(())
}

pub(super) fn plan(
    image: NativeImage,
    table: &str,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeEditPlan, EditError> {
    let source = capture(&image, table, budget, cancelled)?;
    let (records, changed_rows) =
        stage::prepare_indexed_binary(&image, &source, groups, budget, cancelled)?;
    let indexes = index_patches(&image, &source, groups, budget, cancelled)?;
    let mut after = record_serialization::copy(image.as_bytes(), budget, cancelled)?;
    for patch in records {
        apply(
            &mut after,
            patch.offset,
            &patch.before,
            &patch.after,
            budget,
            cancelled,
        )?;
    }
    for patch in indexes {
        apply(
            &mut after,
            patch.offset,
            &patch.before,
            &patch.after,
            budget,
            cancelled,
        )?;
    }
    validation::expected(&image, table, &source.table, &after, budget, cancelled)?;
    let scope = image.catalog().catalog().scope();
    let plan = NativeEditPlan {
        before: image.bytes,
        after,
        changed_rows,
    };
    proof::prove(&plan, scope, table, groups, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(plan)
}

fn apply(
    after: &mut [u8],
    offset: usize,
    before: &[u8],
    value: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, (before.len() + value.len()) as u64)
        .map_err(InspectError::from)?;
    let end = offset
        .checked_add(before.len())
        .ok_or(EditError::Preservation)?;
    let target = after.get_mut(offset..end).ok_or(EditError::Preservation)?;
    if before.len() != value.len() || target != before {
        return Err(EditError::Preservation);
    }
    target.copy_from_slice(value);
    Ok(())
}
