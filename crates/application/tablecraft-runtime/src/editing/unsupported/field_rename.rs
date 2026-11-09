//! Unsupported write hosts retain exact read metadata without rename support.
mod review;
pub(in crate::editing) use review::{State, review};

use crate::{Error, controls::check, document::Document};
use tablecraft_builders::table::field_rename::FieldRenameNameProfile;
use tablecraft_core::limits::ResourceBudget;

pub(in crate::editing) fn source_profile(
    _document: &mut Document,
    _table: &str,
    _column: &str,
    _native_number: usize,
    _budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<FieldRenameNameProfile>, Error> {
    check(cancelled)?;
    Ok(None)
}
