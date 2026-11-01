//! Single-page declaration ownership composed with the shared raw validator.
mod raw;

use super::{
    indexes,
    inline::InlineBitmap,
    ledger::{Ledger, Role as PageRole},
    refused,
};
use crate::{
    EditError, InspectError, NativeImage, TableSchema,
    format::{
        bytes::{u16_at, u32_at},
        layout::Layout,
        reader::Reader,
    },
    header::check,
    inspect_table_indexes,
    table::long_value::maps,
};
use tablecraft_core::limits::ResourceBudget;

const LVAL_COLUMN_OFFSET: usize = 0;
const LVAL_OWNED_OFFSET: usize = size_of::<u16>();
const LVAL_CANDIDATE_OFFSET: usize = LVAL_OWNED_OFFSET + size_of::<u32>();
const LVAL_BYTES: usize = LVAL_CANDIDATE_OFFSET + size_of::<u32>();
const CATALOG_FIRST_LVAL: usize = 5;
const CATALOG_LVAL_COUNT: usize = 4;
const TERMINATOR: u16 = u16::MAX;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    Catalog,
    Relationships,
    Aces,
    User,
    // Target-local declaration closure; the global closed-content family keeps
    // its existing no-user-LVAL role policy.
    SelectedUser,
    // Action-only native ordinals and stored IDs; old UPDATE stays canonical.
    SelectedActions,
}

pub(super) struct Table<'image> {
    pub layout: Layout,
    pub owned: InlineBitmap<'image>,
}

pub(super) fn capture<'image>(
    image: &'image NativeImage,
    ledger: &mut Ledger<'image>,
    number: u32,
    role: Role,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Table<'image>, EditError> {
    check(budget, cancelled)?;
    ledger.claim(number, PageRole::Definition, budget, cancelled)?;
    let declaration = raw::capture(image, number, role, budget, cancelled)?;
    let raw::Declaration {
        layout,
        length,
        candidate,
        mut position,
    } = declaration;
    let page = ledger.page(number)?;
    if layout.physical_indexes != 0 || layout.logical_indexes != 0 {
        position = indexes::capture(image, ledger, number, &layout, position, budget, cancelled)?;
    }
    if role == Role::Catalog {
        let mut seen = [false; CATALOG_LVAL_COUNT];
        for _ in 0..CATALOG_LVAL_COUNT {
            check(budget, cancelled)?;
            let column = usize::from(u16_at(page, position + LVAL_COLUMN_OFFSET)?);
            let index = column
                .checked_sub(CATALOG_FIRST_LVAL)
                .filter(|index| *index < CATALOG_LVAL_COUNT)
                .ok_or_else(refused)?;
            if seen[index] {
                return Err(refused());
            }
            seen[index] = true;
            let owned = ledger.map(
                u32_at(page, position + LVAL_OWNED_OFFSET)?,
                budget,
                cancelled,
            )?;
            let candidate = ledger.map(
                u32_at(page, position + LVAL_CANDIDATE_OFFSET)?,
                budget,
                cancelled,
            )?;
            if !owned.is_empty(budget, cancelled)? || !candidate.is_empty(budget, cancelled)? {
                return Err(refused());
            }
            position += LVAL_BYTES;
        }
    }
    if position + size_of::<u16>() != length || u16_at(page, position)? != TERMINATOR {
        return Err(refused());
    }
    let owned = ledger.map(layout.usage, budget, cancelled)?;
    let candidates = ledger.map(candidate, budget, cancelled)?;
    if !candidates.is_subset_of(owned, budget, cancelled)? {
        return Err(refused());
    }
    Ok(Table { layout, owned })
}

/// Shared raw user declaration closure, without manufacturing global roles.
/// Source remains responsible for map/index membership and physical exclusion.
pub(in crate::table::mutation) fn validate_user(
    image: &NativeImage,
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    validate_selected(image, schema, Role::SelectedUser, budget, cancelled)
}

/// Action-only complete declaration framing; no mutation permission is returned.
pub(in crate::table::mutation) fn validate_actions(
    image: &NativeImage,
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    validate_selected(image, schema, Role::SelectedActions, budget, cancelled)
}

fn validate_selected(
    image: &NativeImage,
    schema: &TableSchema,
    role: Role,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let declaration = raw::capture(image, schema.definition_page(), role, budget, cancelled)?;
    let inspection = inspect_table_indexes(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        schema.name(),
        budget,
        cancelled,
    )?;
    if inspection.schema().object_id() != schema.object_id()
        || inspection.schema().definition_page() != schema.definition_page()
        || inspection.indexes().len() != declaration.layout.logical_indexes
    {
        return Err(refused());
    }
    let position = if matches!(role, Role::SelectedActions) {
        indexes::declaration::validate_actions(
            image,
            inspection.schema(),
            inspection.indexes().iter(),
            budget,
            cancelled,
        )?
    } else {
        indexes::declaration::validate_selected(
            image,
            inspection.schema(),
            inspection.indexes().iter(),
            budget,
            cancelled,
        )?
    };
    let mut reader_source = image.reader();
    let mut reader = Reader {
        source: &mut reader_source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let declarations = maps::declarations(&mut reader, schema)?;
    super::super::no_required_programs::declarations::validate(
        schema,
        &declarations,
        reader.budget,
        reader.cancelled,
    )?;
    // The shared reader may stop at the first terminator. Exact declared tail
    // size additionally refuses omitted pairs, early FFFF and trailing bytes.
    let terminal = declarations
        .entries
        .len()
        .checked_mul(LVAL_BYTES)
        .and_then(|bytes| position.checked_add(bytes))
        .ok_or(InspectError::StructureLimit)?;
    if terminal.checked_add(size_of::<u16>()) != Some(declaration.length) {
        return Err(refused());
    }
    let page = raw::page(image, schema.definition_page())?;
    if u16_at(page, terminal)? != TERMINATOR {
        return Err(refused());
    }
    check(budget, cancelled)?;
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod selected_lval;
