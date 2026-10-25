//! Inert table/query entries charged to the source's selected native allowance.
use super::{Entry, saved};
use std::cell::{OnceCell, RefCell};
use tablecraft_ace::{InspectError, NativeReadSource, inspect_catalog};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::{CatalogScope, ObjectKind};

pub(super) fn read(
    source: &mut impl NativeReadSource,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<Entry>, Vec<saved::QueryEntry>), InspectError> {
    let catalog = inspect_catalog(source, scope, budget, cancelled)?;
    let queries = saved::entries(&catalog, budget, cancelled)?;
    let count = catalog
        .catalog()
        .objects()
        .iter()
        .filter(|o| o.kind() == ObjectKind::Table)
        .count();
    budget.charge_allocation(count as u64, std::mem::size_of::<Entry>() as u64)?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    for o in catalog
        .catalog()
        .objects()
        .iter()
        .filter(|o| o.kind() == ObjectKind::Table)
    {
        if cancelled() {
            return Err(InspectError::Cancelled);
        }
        let name = o.name().as_str();
        budget.charge_allocation(name.len() as u64, 1)?;
        let mut owned = String::new();
        owned
            .try_reserve_exact(name.len())
            .map_err(|_| InspectError::Allocation)?;
        owned.push_str(name);
        entries.push(Entry {
            name: owned,
            columns: OnceCell::new(),
            table: OnceCell::new(),
            index: None,
            projected: RefCell::new(Vec::new()),
            multivalues: OnceCell::new(),
            collections: OnceCell::new(),
        });
    }
    Ok((entries, queries))
}
