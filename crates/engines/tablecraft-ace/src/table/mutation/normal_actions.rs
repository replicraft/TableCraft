//! Fresh target-local action Source; UPDATE retains its canonical capture path.
use super::{
    no_required_programs::NoRequiredTablePrograms,
    preserved_source::{self, Source},
};
use crate::{EditError, NativeImage, header::check, inspect_table_schema};
use tablecraft_core::limits::ResourceBudget;

pub(in crate::table::mutation) fn capture(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    check(budget, cancelled)?;
    let schema = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    let certificate = NoRequiredTablePrograms::capture_actions(image, &schema, budget, cancelled)?;
    preserved_source::capture_no_required_actions(image, table, &certificate, budget, cancelled)
}

/// Current deletion has its own absence purpose and historical-record coverage.
/// Ordinary actions retain their original strict source and record language.
#[cfg(feature = "ace16-create")]
pub(in crate::table::mutation) fn capture_current_delete(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    check(budget, cancelled)?;
    let schema = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    let certificate =
        NoRequiredTablePrograms::capture_current_delete(image, &schema, budget, cancelled)?;
    preserved_source::capture_current_delete(image, table, &certificate, budget, cancelled)
}

/// Retained deletion has its own absence purpose and dynamic ownership coverage.
/// Ordinary actions retain their original strict source and record language.
#[cfg(feature = "ace16-create")]
pub(in crate::table::mutation) fn capture_retained_delete(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    check(budget, cancelled)?;
    let schema = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    let certificate =
        NoRequiredTablePrograms::capture_retained_delete(image, &schema, budget, cancelled)?;
    preserved_source::capture_retained_delete(image, table, &certificate, budget, cancelled)
}
