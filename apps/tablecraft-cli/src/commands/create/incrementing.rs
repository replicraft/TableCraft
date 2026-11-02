//! Explicit selector adaptation; native declaration and row policy stay in ACE.
use super::{Error, tick};
use tablecraft_access::creation::CreationError;
use tablecraft_ace::{CreateColumn, InspectError};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn bind(
    columns: &mut [CreateColumn<'_>],
    selector: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    let mut selected = None;
    for (position, column) in columns.iter().enumerate() {
        tick(budget, cancelled)?;
        budget
            .charge(
                Resource::Steps,
                column.name().len().max(selector.len()) as u64,
            )
            .map_err(InspectError::from)?;
        if column.name().eq_ignore_ascii_case(selector) && selected.replace(position).is_some() {
            return Err(Error::Input(
                "Incrementing selector matches more than one column",
            ));
        }
    }
    let selected = selected.ok_or(Error::Input(
        "Incrementing selector does not match a declared column",
    ))?;
    if columns[selected].primary_key_name().is_none() {
        return Err(Error::Input(
            "Incrementing selector must name the explicit primary column",
        ));
    }
    columns[selected] = columns[selected]
        .with_incrementing_autonumber()
        .map_err(|cause| Error::Creation(CreationError::Create(cause)))?;
    Ok(())
}
