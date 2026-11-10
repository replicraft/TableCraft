//! Full supported content ownership, without creator or empty-inventory trust.
mod catalog;
mod data;
pub(super) mod definition;
mod header;
mod indexes;
mod inline;
mod ledger;
mod records;
#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub(super) mod schema_dependencies;

use crate::{EditError, NativeImage, TableSchema, WriteBlocker, header::check, read_table};
use ledger::Ledger;
use tablecraft_core::{ids::ObjectId, limits::ResourceBudget};

/// Source-owned closed-content result. It has no public constructor, creator
/// marker or stored permission flag. Normal write admission is not enabled by
/// this first source-validation slice.
pub(in crate::table::mutation) struct ClosedContent<'image> {
    image: &'image NativeImage,
    users: Vec<(ObjectId, u32)>,
}

impl ClosedContent<'_> {
    /// The actual image and actual user-table role must both match the proof.
    pub(in crate::table::mutation) fn require_source(
        &self,
        image: &NativeImage,
        schema: &TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), EditError> {
        check(budget, cancelled)?;
        if !std::ptr::eq(self.image, image) {
            return Err(EditError::Preservation);
        }
        for &(object, definition) in &self.users {
            check(budget, cancelled)?;
            if object == schema.object_id() && definition == schema.definition_page() {
                return Ok(());
            }
        }
        Err(EditError::Preservation)
    }
}

pub(in crate::table::mutation) fn capture<'image>(
    image: &'image NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ClosedContent<'image>, EditError> {
    let _span = tracing::debug_span!("native.ordinary_content.capture").entered();
    image.require_standard_capture()?;
    header::validate(image, budget, cancelled)?;
    let mut ledger = Ledger::new(image, budget, cancelled)?;
    let objects = catalog::capture(image, &mut ledger, budget, cancelled)?;
    for (number, role) in objects.systems {
        let table = definition::capture(image, &mut ledger, number, role, budget, cancelled)?;
        if table.layout.records != 0 || !table.owned.is_empty(budget, cancelled)? {
            return Err(refused());
        }
    }
    for &(object, number) in &objects.users {
        check(budget, cancelled)?;
        let table = definition::capture(
            image,
            &mut ledger,
            number,
            definition::Role::User,
            budget,
            cancelled,
        )?;
        data::validate(&mut ledger, number, &table, budget, cancelled)?;
        let mut descriptor = None;
        for candidate in image.catalog().catalog().objects() {
            check(budget, cancelled)?;
            if candidate.id() == object {
                descriptor = Some(candidate);
                break;
            }
        }
        let descriptor = descriptor.ok_or_else(refused)?;
        let read = read_table(
            &mut image.reader(),
            image.catalog().catalog().scope(),
            descriptor.name().as_str(),
            budget,
            cancelled,
        )?;
        if read.schema().object_id() != object
            || read.schema().definition_page() != number
            || read.rows().len() != table.layout.records as usize
            || read.columns().len() != table.layout.fields.len()
        {
            return Err(refused());
        }
    }
    ledger.finish(budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(ClosedContent {
        image,
        users: objects.users,
    })
}

fn refused() -> EditError {
    EditError::Unsupported(WriteBlocker::ActiveContent)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod dynamic_tests;

#[cfg(test)]
mod counter_index_tests;

#[cfg(test)]
mod counter_index_resource_tests;
