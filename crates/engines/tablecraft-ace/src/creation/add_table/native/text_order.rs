//! Destination root default for newly authored textual declarations.
use super::super::AddTableError;
use crate::{
    CreateColumn, NativeImage, NativeSortOrder, header::check, inspect_database_sort_order,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

pub(super) fn capture(
    image: &NativeImage,
    columns: &[CreateColumn<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<NativeSortOrder>, AddTableError> {
    let mut has_text = false;
    for column in columns {
        check(budget, cancelled)?;
        has_text |= column.scalar_type() == ScalarType::Text;
    }
    if !has_text {
        return Ok(None);
    }
    let order = inspect_database_sort_order(&mut image.reader(), budget, cancelled)?;
    // This construction profile qualifies the exact two General variants.
    // Other stored defaults remain observable metadata, never2010 fallback.
    if order.general_family().is_none() {
        return Err(AddTableError::Source);
    }
    Ok(Some(order))
}
