//! Selected-field facts layered on the shared current-table schema proof.
use super::{FieldRenameError as Error, source::Qualified};
use crate::{
    NativeImage,
    format::row::reference_slot,
    table::mutation::{
        current_schema,
        ordinary_content::definition::{self, RowDeclaration},
        saved_field::Observed,
    },
};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn capture(
    image: NativeImage,
    table: &str,
    column: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Qualified, Error> {
    let proof =
        current_schema::Source::capture(image, table, budget, cancelled).map_err(source_error)?;
    let observed = selected(proof.image(), proof.table(), column, budget, cancelled)?;
    Ok(Qualified::from_current(proof.into_image(), observed))
}

pub(super) fn observe(
    image: &NativeImage,
    table: &str,
    field: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Observed, Error> {
    let current = current_schema::observe(image, table, budget, cancelled).map_err(source_error)?;
    selected(image, &current, field, budget, cancelled)
}

fn selected(
    image: &NativeImage,
    current: &current_schema::Table,
    field: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Observed, Error> {
    // Profile four retains its original declaration language even though the
    // shared current-schema observer now supports additional metadata history.
    // The retained-storage profile selects its separate proof before admission.
    definition::validate_rows(
        image,
        &current.schema,
        RowDeclaration::Actions,
        budget,
        cancelled,
    )?;
    let observed = Observed::capture(image, current.schema.name(), field, budget, cancelled)?;
    let current_schema::UserStorage::Single { reference, .. } = &current.user_storage else {
        return Err(Error::Source);
    };
    // This operation's packing still moves the user row and following OFF row.
    // The shared source proof itself does not require a particular live-row order.
    if current.schema.object_id() != observed.schema.object_id()
        || *reference != observed.property_reference
        || usize::from(reference_slot(current.database_reference))
            != usize::from(reference_slot(*reference)) + 1
    {
        return Err(Error::Source);
    }
    Ok(observed)
}

pub(super) fn source_error(error: current_schema::Error) -> Error {
    match error {
        current_schema::Error::Inspect(error) => Error::Inspect(error),
        current_schema::Error::Native(error) => Error::RowSource(error),
        current_schema::Error::Source => Error::Source,
    }
}

#[cfg(test)]
mod tests;
