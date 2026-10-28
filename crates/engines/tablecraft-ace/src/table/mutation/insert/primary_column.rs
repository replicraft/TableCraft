//! Resolve the primary key's native column independently of display/type order.
use crate::{EditError, TableCheck, WriteBlocker, header::check};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

/// This selects a source column, not write authority. Primary maintenance still
/// qualifies the complete physical index and every logical alias separately.
pub(super) fn resolve(
    checked: &TableCheck,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    check(budget, cancelled)?;
    let mut selected = None;

    for entry in checked.indexes() {
        check(budget, cancelled)?;
        let index = entry.index();
        let Some(constraints) = index.constraints() else {
            continue;
        };
        if !constraints.primary_key() {
            continue;
        }

        let [component] = index.columns() else {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        };
        if component.descending() || selected.is_some() {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }

        for (position, column) in checked.schema().columns().iter().enumerate() {
            check(budget, cancelled)?;
            if column.number() != component.number() {
                continue;
            }
            if column.scalar_type() != Some(ScalarType::Int32) {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
            selected = Some(position);
            break;
        }
        if selected.is_none() {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }

    selected.ok_or(EditError::Unsupported(WriteBlocker::IndexedColumn))
}
