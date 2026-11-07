//! Explicit fresh native construction; no imported-source edit permission.
mod args;
mod error;
mod incrementing;
mod input;
mod primary;
mod report;
mod schema;

#[cfg(all(test, unix))]
mod tests;

pub(crate) use args::Arguments;
pub(crate) use error::Error;

use std::io::Write;
use tablecraft_access::creation::{CreationDestination, PreparedCreation};
use tablecraft_ace::{CreateColumn, InspectError};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

pub(crate) fn execute(
    arguments: Arguments,
    password: Option<&str>,
    writer: &mut dyn Write,
) -> Result<(), Error> {
    let _span = tracing::debug_span!("cli.create_database").entered();
    if password.is_some() {
        return Err(Error::Password);
    }
    if arguments.incrementing.is_some() && arguments.primary_key.is_empty() {
        return Err(Error::Input(
            "--incrementing requires an explicit --primary-key",
        ));
    }
    let mut budget = arguments.budget();
    let cancelled = || false;
    if arguments.blank {
        let prepared = CreationDestination::capture(&arguments.path, &mut budget, &cancelled)
            .map_err(Error::Creation)?
            .prepare_blank(&mut budget, &cancelled)
            .map_err(Error::Creation)?;
        return finish(prepared, &mut budget, &cancelled, writer);
    }
    let table = arguments.table.as_deref().ok_or(Error::Input(
        "--table is required unless --blank is selected",
    ))?;
    let typed = if arguments.fields.is_empty() {
        None
    } else {
        Some(schema::parse(&arguments.fields, &mut budget, &cancelled)?)
    };
    // Resolve optional key metadata before destination capture or TSV reads.
    // The engine still owns name/type/key constraints and physical encoding.
    let primary_columns = if arguments.primary_key.is_empty() {
        None
    } else {
        let mut columns = Vec::new();
        if let Some(fields) = &typed {
            input::reserve(&mut columns, fields.len(), &mut budget)?;
            columns.extend(fields.iter().map(|field| field.column));
        } else {
            input::reserve(&mut columns, arguments.columns.len(), &mut budget)?;
            columns.extend(
                arguments
                    .columns
                    .iter()
                    .map(|name| CreateColumn::new(name, ScalarType::Int32)),
            );
        }
        primary::bind(
            &mut columns,
            &arguments.primary_key,
            &mut budget,
            &cancelled,
        )?;
        if let Some(selector) = arguments.incrementing.as_deref() {
            incrementing::bind(&mut columns, selector, &mut budget, &cancelled)?;
        }
        Some(columns)
    };
    let destination = CreationDestination::capture(&arguments.path, &mut budget, &cancelled)
        .map_err(Error::Creation)?;
    let prepared = if let Some(fields) = typed {
        let rows = if let Some(path) = &arguments.data {
            let bytes = input::capture(path, &mut budget, &cancelled)?;
            input::typed_rows(&bytes, &fields, &mut budget, &cancelled)?
        } else {
            Vec::new()
        };
        let columns = if let Some(columns) = primary_columns {
            columns
        } else {
            let mut columns = Vec::new();
            input::reserve(&mut columns, fields.len(), &mut budget)?;
            columns.extend(fields.iter().map(|field| field.column));
            columns
        };
        let mut borrowed_rows = Vec::new();
        input::reserve(&mut borrowed_rows, rows.len(), &mut budget)?;
        borrowed_rows.extend(rows.iter().map(Vec::as_slice));
        destination
            .prepare_typed(table, &columns, &borrowed_rows, &mut budget, &cancelled)
            .map_err(Error::Creation)?
    } else {
        let rows = if let Some(path) = &arguments.data {
            let bytes = input::capture(path, &mut budget, &cancelled)?;
            input::rows(&bytes, &arguments.columns, &mut budget, &cancelled)?
        } else {
            Vec::new()
        };
        if let Some(columns) = primary_columns {
            // Retain the strict --column parser. Only this new keyed route
            // adapts its already validated Int32 cells to the typed service.
            let values = primary::values(&rows, &mut budget, &cancelled)?;
            let mut borrowed_rows = Vec::new();
            input::reserve(&mut borrowed_rows, values.len(), &mut budget)?;
            borrowed_rows.extend(values.iter().map(Vec::as_slice));
            destination
                .prepare_typed(table, &columns, &borrowed_rows, &mut budget, &cancelled)
                .map_err(Error::Creation)?
        } else {
            let mut columns = Vec::new();
            input::reserve(&mut columns, arguments.columns.len(), &mut budget)?;
            columns.extend(arguments.columns.iter().map(String::as_str));
            let mut borrowed_rows = Vec::new();
            input::reserve(&mut borrowed_rows, rows.len(), &mut budget)?;
            borrowed_rows.extend(rows.iter().map(Vec::as_slice));
            destination
                .prepare(table, &columns, &borrowed_rows, &mut budget, &cancelled)
                .map_err(Error::Creation)?
        }
    };
    finish(prepared, &mut budget, &cancelled, writer)
}

fn finish(
    prepared: PreparedCreation,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
    writer: &mut dyn Write,
) -> Result<(), Error> {
    let report = report::Report::prepare(&prepared, budget)?;
    let receipt = prepared
        .publish(budget, cancelled)
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
