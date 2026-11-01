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
