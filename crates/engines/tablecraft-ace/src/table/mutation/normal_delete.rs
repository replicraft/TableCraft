//! Fixed-length ordinary DELETE with fresh program absence and raw-row proof.
#[cfg(feature = "ace16-create")]
pub(super) mod current;
#[cfg(feature = "ace16-create")]
pub(super) mod retained;

mod indexed;
mod proof;
mod selection;
mod stage;

#[cfg(test)]
mod tests;

use crate::{
    EditError, InspectError, NativeEditPlan, NativeImage, RowDelete, WriteBlocker,
    header::check,
    inspect_table_schema,
    table::mutation::{fixed_records, normal_actions, preserved_source::Source, rows},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const NATIVE_CATALOG_FIELDS: usize = 17;
const CATALOG_NAME: &str = "MSysObjects";

impl NativeImage {
    /// Plan exact original ordinary-row deletions without reclaiming payloads.
    pub(crate) fn plan_ordinary_row_deletes(
        self,
        table: &str,
        deletes: &[RowDelete],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeEditPlan, EditError> {
        plan(self, table, deletes, Family::Ordinary, budget, cancelled)
    }
}

#[derive(Clone, Copy)]
enum Family {
    Ordinary,
    #[cfg(feature = "ace16-create")]
    Current,
    #[cfg(feature = "ace16-create")]
    TableDrop,
    #[cfg(feature = "ace16-create")]
    Retained,
}

impl Family {
    fn capture(
        self,
        image: &NativeImage,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Source, EditError> {
        match self {
            Self::Ordinary => capture(image, table, budget, cancelled),
            #[cfg(feature = "ace16-create")]
            Self::Current => current::capture(image, table, budget, cancelled),
            #[cfg(feature = "ace16-create")]
            Self::TableDrop => current::capture_table_drop(image, table, budget, cancelled),
            #[cfg(feature = "ace16-create")]
            Self::Retained => retained::capture(image, table, budget, cancelled),
        }
    }

    fn is_retained(self) -> bool {
        match self {
            #[cfg(feature = "ace16-create")]
            Self::Retained => true,
            _ => false,
        }
    }

    fn roots<'u, U: rows::RowAssignment>(
        self,
        image: &NativeImage,
        schema: &crate::TableSchema,
        requests: &'u [U],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<super::record_roots::Roots<'u, U>, EditError> {
        match self {
            #[cfg(feature = "ace16-create")]
            Self::Retained => {
                super::record_roots::capture_retained(image, schema, requests, budget, cancelled)
            }
            _ => super::record_roots::capture(image, schema, requests, budget, cancelled),
        }
    }

    fn image(
        self,
        bytes: &[u8],
        scope: tablecraft_model::CatalogScope,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeImage, EditError> {
        match self {
            #[cfg(feature = "ace16-create")]
            Self::Retained => Ok(NativeImage::capture_retained(
                &mut std::io::Cursor::new(bytes),
                scope,
                budget,
                cancelled,
            )?),
            _ => Ok(NativeImage::capture(
                &mut std::io::Cursor::new(bytes),
                scope,
                budget,
                cancelled,
            )?),
        }
    }

    fn long_values(
        self,
        image: &NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<super::lval_ownership::Inventory, EditError> {
        match self {
            #[cfg(feature = "ace16-create")]
            Self::Retained => {
                super::lval_ownership::Inventory::capture_retained(image, budget, cancelled)
            }
            _ => super::lval_ownership::Inventory::capture(image, budget, cancelled),
        }
    }

    fn unique<U: rows::RowAssignment>(
        self,
        requests: &[U],
        source_rows: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        match (self, requests.len() > source_rows) {
            (Self::Ordinary, _) => rows::unique(requests, budget, cancelled),
            #[cfg(feature = "ace16-create")]
            (Self::Current | Self::TableDrop | Self::Retained, exceeds_source) => {
                check(budget, cancelled)?;
                if exceeds_source {
                    return Err(EditError::Conflict);
                }
                rows::unique_identities(requests, budget, cancelled)
            }
        }
    }
}

fn plan(
    image: NativeImage,
    table: &str,
    deletes: &[RowDelete],
    family: Family,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeEditPlan, EditError> {
    let _span = tracing::debug_span!("native.row.delete.plan").entered();
    let source = family.capture(&image, table, budget, cancelled)?;
    family.unique(deletes, source.records.len(), budget, cancelled)?;
    for request in deletes {
        check(budget, cancelled)?;
        if request.expected_bytes().len() > crate::format::page::PAGE_BYTES {
            return Err(InspectError::StructureLimit.into());
        }
    }

    let after = compose(&image, &source, deletes, family, budget, cancelled)?;
    let scope = image.catalog().catalog().scope();
    let plan = NativeEditPlan {
        before: image.bytes,
        after,
        changed_rows: deletes.len(),
    };
    proof::prove_with_source(&plan, scope, table, deletes, family, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(plan)
}

fn compose(
    image: &NativeImage,
    source: &Source,
    deletes: &[RowDelete],
    family: Family,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let staged = stage::prepare(image, source, deletes, family, budget, cancelled)?;
    let indexes = if source.indexes.indexes().is_empty() {
        Vec::new()
    } else {
        indexed::patches(image, source, deletes, family, budget, cancelled)?
    };
    let mut after = fixed_records::copy(image.as_bytes(), budget, cancelled)?;
    staged.apply(&mut after, budget, cancelled)?;
    for patch in indexes {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, patch.before.len() as u64)
            .map_err(InspectError::from)?;
        let end = patch
            .offset
            .checked_add(patch.before.len())
            .ok_or(EditError::Preservation)?;
        let output = after
            .get_mut(patch.offset..end)
            .ok_or(EditError::Preservation)?;
        if output != patch.before {
            return Err(EditError::Preservation);
        }
        output.copy_from_slice(&patch.after);
    }

    Ok(after)
}

fn capture(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    // Global closed-content DATA framing currently excludes tombstones. Keep
    // that separate profile unchanged: this action consumes the actual native
    // target-local NoRequired certificate, selected from original metadata.
    let catalog = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        CATALOG_NAME,
        budget,
        cancelled,
    )?;
    if catalog.columns().len() != NATIVE_CATALOG_FIELDS {
        return Err(EditError::Unsupported(WriteBlocker::ActiveContent));
    }
    let source = normal_actions::capture(image, table, budget, cancelled)?;
    if !source.indexes.indexes().is_empty() {
        indexed::qualify(image, &source, Family::Ordinary, budget, cancelled)?;
    }
    // Source capture already rejects every relationship involving this table.
    // Unrelated declarations remain protected and do not authorize cascades.
    check(budget, cancelled)?;
    Ok(source)
}

#[cfg(test)]
mod node_sources;

#[cfg(test)]
mod node_delete_sources;

#[cfg(test)]
mod integral_tests;

#[cfg(test)]
mod sparse_numeric_tests;

#[cfg(test)]
mod binary_tests;

#[cfg(test)]
mod node_retirement_sources;

#[cfg(test)]
mod node_retirement_tests;

#[cfg(test)]
mod recursive_node_sources;

#[cfg(test)]
mod recursive_retirement_sources;

#[cfg(test)]
mod recursive_retirement_tests;

#[cfg(test)]
mod repeated_retirement_sources;

#[cfg(test)]
mod repeated_retirement_tests;

#[cfg(test)]
mod tail_chain_sources;
