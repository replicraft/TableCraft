//! Explicit fresh native construction; no imported-source edit permission.
mod args;
mod error;
mod input;
mod report;

#[cfg(all(test, unix))]
mod tests;

pub(crate) use args::Arguments;
pub(crate) use error::Error;

use std::io::Write;
use tablecraft_access::creation::CreationDestination;
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(crate) fn execute(
    arguments: Arguments,
    password: Option<&str>,
    writer: &mut dyn Write,
) -> Result<(), Error> {
    let _span = tracing::debug_span!("cli.create_database").entered();
    if password.is_some() {
        return Err(Error::Password);
    }
    let mut budget = arguments.budget();
    let cancelled = || false;
    let destination = CreationDestination::capture(&arguments.path, &mut budget, &cancelled)
        .map_err(Error::Creation)?;
    let rows = if let Some(path) = &arguments.data {
        let bytes = input::capture(path, &mut budget, &cancelled)?;
        input::rows(&bytes, &arguments.columns, &mut budget, &cancelled)?
    } else {
        Vec::new()
    };
    let mut columns = Vec::new();
    input::reserve(&mut columns, arguments.columns.len(), &mut budget)?;
    columns.extend(arguments.columns.iter().map(String::as_str));
    let mut borrowed_rows = Vec::new();
    input::reserve(&mut borrowed_rows, rows.len(), &mut budget)?;
    borrowed_rows.extend(rows.iter().map(Vec::as_slice));
    let prepared = destination
        .prepare(
            &arguments.table,
            &columns,
            &borrowed_rows,
            &mut budget,
            &cancelled,
        )
        .map_err(Error::Creation)?;
    let report = report::Report::prepare(&prepared, &mut budget)?;
    let receipt = prepared
        .publish(&mut budget, &cancelled)
        .map_err(Error::Creation)?;
    report.publish(receipt, writer)
}

fn tick(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), InspectError> {
    if cancelled() {
        return Err(InspectError::Cancelled);
    }
    budget
        .charge(Resource::Steps, 1)
        .map_err(InspectError::from)
}
