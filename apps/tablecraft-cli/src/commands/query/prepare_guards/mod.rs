//! Explicit SQL-selected guard preparation; no copy publication or normal save.
mod args;
mod report;
#[cfg(test)]
mod tests;

pub(crate) use args::Arguments;

use crate::{native, output::OutputError};
use std::fmt;
use tablecraft_access::{
    AccessInputError,
    engineering::guards::{self, GuardSqlError},
};
use tablecraft_ace::{BeforeEventGuardSelector, InspectError, NativeImage, inspect_header};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_query::QueryError;
use tablecraft_sql::{ParseError, ParseLimits, parse_query};

pub(in crate::commands) enum Error {
    Parse(ParseError),
    Query(QueryError),
    Source(AccessInputError),
    Native(InspectError),
    Guard(GuardSqlError),
    Output(OutputError),
    Encrypted,
    Deployment,
    Selection,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => error.fmt(formatter),
            Self::Query(error) => error.fmt(formatter),
            Self::Source(error) => error.fmt(formatter),
            Self::Native(error) => error.fmt(formatter),
            Self::Guard(error) => error.fmt(formatter),
            Self::Output(error) => error.fmt(formatter),
            Self::Encrypted => formatter
                .write_str("encrypted sources are outside the explicit before-event guard profile"),
            Self::Deployment => formatter.write_str(
                "deployment packages are outside the explicit before-event guard profile",
            ),
            Self::Selection => {
                formatter.write_str("expected one or more bounded --guard-property KIND NAME pairs")
            }
        }
    }
}

pub(super) fn run(args: Arguments, _password: Option<&str>) -> Result<String, Error> {
    let _span = tracing::debug_span!("cli.query.prepare_guards").entered();
    let mut budget = ResourceBudget::new(args.work_bytes, args.work_rows, args.work_steps);
    let selectors = selectors(&args.guard_properties, &mut budget)?;
    budget
        .charge_allocation(args.sql.len() as u64, 1)
        .map_err(InspectError::from)
        .map_err(Error::Native)?;
    budget
        .charge(Resource::Steps, args.sql.len() as u64)
        .map_err(InspectError::from)
        .map_err(Error::Native)?;
    let query = parse_query(&args.sql, ParseLimits::default()).map_err(Error::Parse)?;

    // Closed statement shape is checked before opening any source. Binding and
    // SQL selection remain query-owned; stored macro expressions are engine-owned.
    guards::local_guard_table(query.query(), &mut budget, &|| false).map_err(Error::Query)?;
    let mut source = native::open_raw(&args.database, &mut budget).map_err(Error::Source)?;
    if source.is_deployment() {
        return Err(Error::Deployment);
    }
    if inspect_header(&mut source, &mut budget, &|| false)
        .map_err(Error::Native)?
        .encoded_pages()
    {
        return Err(Error::Encrypted);
    }
    let image = NativeImage::capture(&mut source, native::CATALOG_SCOPE, &mut budget, &|| false)
        .map_err(Error::Native)?;

    let prepared = guards::prepare(image, query.query(), &selectors, &mut budget, None, &|| {
        false
    })
    .map_err(Error::Guard)?;
    let text = report::format(&prepared, args.trace, &mut budget)?;
    // Keep the same raw source guard through preparation and report formatting.
    // Returning this completed presentation never authorizes candidate publication.
    drop(source);
    Ok(text)
}

fn selectors<'a>(
    values: &'a [String],
    budget: &mut ResourceBudget,
) -> Result<Vec<BeforeEventGuardSelector<'a>>, Error> {
    if values.is_empty()
        || !values.len().is_multiple_of(2)
        || values.len() / 2 > NativeImage::MAX_BEFORE_GUARD_PROPERTIES
    {
        return Err(Error::Selection);
    }
    budget
        .charge_allocation(
            (values.len() / 2) as u64,
            size_of::<BeforeEventGuardSelector<'_>>() as u64,
        )
        .map_err(InspectError::from)
        .map_err(Error::Native)?;
    let mut selected = Vec::new();
    selected
        .try_reserve_exact(values.len() / 2)
        .map_err(|_| Error::Native(InspectError::Allocation))?;
    for pair in values.as_chunks::<2>().0 {
        budget
            .charge(Resource::Steps, (pair[0].len() + pair[1].len()) as u64)
            .map_err(InspectError::from)
            .map_err(Error::Native)?;
        let kind = pair[0].parse::<u16>().map_err(|_| Error::Selection)?;
        if pair[1].is_empty() {
            return Err(Error::Selection);
        }
        selected.push(BeforeEventGuardSelector::new(kind, &pair[1]));
    }
    Ok(selected)
}
