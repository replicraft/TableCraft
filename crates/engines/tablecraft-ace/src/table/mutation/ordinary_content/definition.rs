//! Complete declaration ownership composed with the shared raw validator.
mod raw;
pub(super) mod storage;
pub(in crate::table::mutation) mod tail;

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
        definition::LENGTH_OFFSET,
        layout::Layout,
        reader::Reader,
    },
    header::check,
    inspect_table_indexes,
    table::long_value::maps,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

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
    // INSERT-only fixed LONG generation with ordered sparse native identities.
    SelectedGenerated,
    // Current metadata retains retired native ordinals and variable-slot counters.
    #[cfg(feature = "ace16-create")]
    SelectedSchema,
    #[cfg(feature = "ace16-create")]
    SelectedNumericIndexSchema,
}

impl Role {
    pub(super) fn action_ordinals(self) -> bool {
        matches!(self, Self::SelectedActions | Self::SelectedGenerated) || self.current_schema()
    }

    pub(super) fn current_schema(self) -> bool {
        match self {
            #[cfg(feature = "ace16-create")]
            Self::SelectedSchema | Self::SelectedNumericIndexSchema => true,
            _ => false,
        }
    }

    pub(super) fn selected_user(self) -> bool {
        matches!(
            self,
            Self::SelectedUser | Self::SelectedActions | Self::SelectedGenerated
        ) || self.current_schema()
    }

    pub(super) fn user(self) -> bool {
        self == Self::User || self.selected_user()
    }
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
        retained_tail: _,
        storage,
    } = declaration;
    for continuation in storage.pages().skip(1) {
        ledger.claim(continuation, PageRole::Definition, budget, cancelled)?;
    }
    let page: &[u8] = &storage;
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
    validate_selected(
        image,
        schema,
        Role::SelectedUser,
        false,
        TailMode::Strict,
        budget,
        cancelled,
    )
    .map(drop)
}

/// Action-only complete declaration framing; no mutation permission is returned.
pub(in crate::table::mutation) fn validate_actions(
    image: &NativeImage,
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    validate_selected(
        image,
        schema,
        Role::SelectedActions,
        false,
        TailMode::Strict,
        budget,
        cancelled,
    )
    .map(drop)
}

/// Exact sparse fixed-LONG declaration; program absence is still proved separately.
pub(in crate::table::mutation) fn validate_generated(
    image: &NativeImage,
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    validate_selected(
        image,
        schema,
        Role::SelectedGenerated,
        false,
        TailMode::Strict,
        budget,
        cancelled,
    )
    .map(drop)
}

/// Dense canonical field identities with complete readonly scalar index closure.
pub(in crate::table::mutation) fn validate_retained_updates(
    image: &NativeImage,
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    validate_selected(
        image,
        schema,
        Role::SelectedUser,
        true,
        TailMode::Strict,
        budget,
        cancelled,
    )
    .map(drop)
}

#[derive(Clone, Copy)]
enum TailMode {
    Strict,
    RowOnly,
}

/// Closed declaration policies used only by actual normal row operations.
pub(in crate::table::mutation) enum RowDeclaration {
    DenseIntegral,
    DenseRetained,
    Actions,
    SparseGenerated,
    #[cfg(feature = "ace16-create")]
    CurrentSchema,
    #[cfg(feature = "ace16-create")]
    NumericIndexSchema,
}

pub(in crate::table::mutation) fn validate_rows(
    image: &NativeImage,
    schema: &TableSchema,
    declaration: RowDeclaration,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<tail::RetainedDefinitionTail>, EditError> {
    let page = storage::physical_page(image, schema.definition_page())?;
    let length = u32_at(page, LENGTH_OFFSET)? as usize;
    // Joined declarations use the strict path: the raw storage owner checks
    // every continuation's header and padding. Retired root suffixes retain
    // their existing one-page admission and witness.
    let slack = page.get(length.min(page.len())..).ok_or_else(refused)?;
    budget
        .charge(Resource::Steps, slack.len() as u64)
        .map_err(InspectError::from)?;
    check(budget, cancelled)?;
    if slack.iter().all(|byte| *byte == 0) {
        // Select the unchanged strict path from original metadata, never by
        // retrying a failed proof. Existing zero-tail work retains no witness.
        match declaration {
            RowDeclaration::DenseIntegral => validate_user(image, schema, budget, cancelled)?,
            RowDeclaration::DenseRetained => {
                validate_retained_updates(image, schema, budget, cancelled)?
            }
            RowDeclaration::Actions => validate_actions(image, schema, budget, cancelled)?,
            #[cfg(feature = "ace16-create")]
            RowDeclaration::CurrentSchema => validate_selected(
                image,
                schema,
                Role::SelectedSchema,
                false,
                TailMode::Strict,
                budget,
                cancelled,
            )
            .map(drop)?,
            #[cfg(feature = "ace16-create")]
            RowDeclaration::NumericIndexSchema => validate_selected(
                image,
                schema,
                Role::SelectedNumericIndexSchema,
                false,
                TailMode::Strict,
                budget,
                cancelled,
            )
            .map(drop)?,
            RowDeclaration::SparseGenerated => {
                validate_generated(image, schema, budget, cancelled)?
            }
        }
        return Ok(None);
    }

    let (role, retained_updates) = match declaration {
        RowDeclaration::DenseIntegral => (Role::SelectedUser, false),
        RowDeclaration::DenseRetained => (Role::SelectedUser, true),
        RowDeclaration::Actions => (Role::SelectedActions, false),
        RowDeclaration::SparseGenerated => (Role::SelectedGenerated, false),
        #[cfg(feature = "ace16-create")]
        RowDeclaration::CurrentSchema => (Role::SelectedSchema, false),
        #[cfg(feature = "ace16-create")]
        RowDeclaration::NumericIndexSchema => (Role::SelectedNumericIndexSchema, false),
    };
    validate_selected(
        image,
        schema,
        role,
        retained_updates,
        TailMode::RowOnly,
        budget,
        cancelled,
    )
}

fn validate_selected(
    image: &NativeImage,
    schema: &TableSchema,
    role: Role,
    retained_updates: bool,
    tail_mode: TailMode,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<tail::RetainedDefinitionTail>, EditError> {
    check(budget, cancelled)?;
    let declaration = match tail_mode {
        TailMode::Strict => raw::capture(image, schema.definition_page(), role, budget, cancelled)?,
        TailMode::RowOnly => {
            raw::capture_rows(image, schema.definition_page(), role, budget, cancelled)?
        }
    };
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
    #[cfg(feature = "ace16-create")]
    let numeric_index_schema = role == Role::SelectedNumericIndexSchema;
    #[cfg(not(feature = "ace16-create"))]
    let numeric_index_schema = false;
    let position = if numeric_index_schema {
        indexes::declaration::validate_numeric_index_schema(
            image,
            inspection.schema(),
            inspection.indexes().iter(),
            budget,
            cancelled,
        )?
    } else if role == Role::SelectedActions || role.current_schema() {
        indexes::declaration::validate_actions(
            image,
            inspection.schema(),
            inspection.indexes().iter(),
            budget,
            cancelled,
        )?
    } else if role == Role::SelectedGenerated {
        indexes::declaration::validate_generated(
            image,
            inspection.schema(),
            inspection.indexes().iter(),
            budget,
            cancelled,
        )?
    } else if retained_updates {
        indexes::declaration::validate_retained_updates(
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
    if u16_at(&declaration.storage, terminal)? != TERMINATOR {
        return Err(refused());
    }
    check(budget, cancelled)?;
    if declaration.retained_tail {
        tail::RetainedDefinitionTail::capture(
            image,
            schema.definition_page(),
            declaration.length,
            budget,
            cancelled,
        )
        .map(Some)
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod selected_lval;

/// Canonical UPDATE field identities and stored IDs, without widening the old
/// selected-index family. The caller also performs complete action declaration
/// validation through its fresh NoRequired certificate.
pub(in crate::table::mutation) fn validate_binary_updates(
    image: &NativeImage,
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    raw::capture_rows(
        image,
        schema.definition_page(),
        Role::SelectedUser,
        budget,
        cancelled,
    )?;
    if schema.columns().len() != schema.layout.max_columns {
        return Err(refused());
    }
    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if field.number() != position {
            return Err(refused());
        }
    }
    Ok(())
}
