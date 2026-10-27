//! Own bounded request groups before any database capture or output publication.
use super::requests;
use std::{error::Error, ffi::OsString, path::PathBuf};
use tablecraft_ace::{InspectError, NativeImage, RowDelete};
use tablecraft_core::limits::{Resource, ResourceBudget};

const GROUP_ARGUMENTS: usize = 2;

pub(super) struct Group {
    pub table: String,
    pub deletes: Vec<RowDelete>,
}

pub(super) fn load(
    arguments: Vec<OsString>,
    budget: &mut ResourceBudget,
) -> Result<Vec<Group>, Box<dyn Error>> {
    if arguments.is_empty() || !arguments.len().is_multiple_of(GROUP_ARGUMENTS) {
        return Err("Expected one or more --group TABLE DELETE_TSV_PATH pairs".into());
    }
    let count = arguments.len() / GROUP_ARGUMENTS;
    if count > NativeImage::MAX_DELETE_BATCH_TABLES {
        return Err(InspectError::StructureLimit.into());
    }
    budget.charge_allocation(count as u64, size_of::<Group>() as u64)?;
    let mut result: Vec<Group> = Vec::new();
    result.try_reserve_exact(count)?;
    let mut arguments = arguments.into_iter();
    let mut total = 0_usize;
    while let Some(table) = arguments.next() {
        let table = table
            .into_string()
            .map_err(|_| "Table selector must be valid UTF-8")?;
        budget.charge_allocation(table.len() as u64, 1)?;
        budget.charge(Resource::Steps, (result.len() + table.len()) as u64)?;
        if result
            .iter()
            .any(|group| group.table.eq_ignore_ascii_case(&table))
        {
            return Err("Repeated table groups are not allowed".into());
        }
        let path = PathBuf::from(arguments.next().ok_or("Missing delete-request path")?);
        let deletes = requests::Requests::from(path).load(budget)?;
        total = total
            .checked_add(deletes.len())
            .filter(|count| *count <= NativeImage::MAX_DELETE_BATCH_ROWS)
            .ok_or(InspectError::StructureLimit)?;
        result.push(Group { table, deletes });
    }
    Ok(result)
}
