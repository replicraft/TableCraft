//! Global native FROM qualifiers remain inert exact owned syntax.
use super::{SavedSelectError, context::Context};
use tablecraft_ace::SavedQueryRow;
use tablecraft_sql::select::ExternalDatabase;

// Qualified original attribute-4 metadata/getter profile, not all native flags.
const REMOTE_DATABASE_FLAG: i16 = 2;

pub(super) fn validate(row: &SavedQueryRow) -> Result<(), SavedSelectError> {
    if row.flag() != Some(REMOTE_DATABASE_FLAG) || row.extra().is_some() || row.name2().is_some() {
        return Err(SavedSelectError::Unsupported);
    }
    if row.name1().is_none() && row.expression().is_none() {
        return Err(SavedSelectError::InvalidMetadata);
    }
    Ok(())
}

pub(super) fn read(
    row: Option<&SavedQueryRow>,
    has_sources: bool,
    context: &mut Context<'_>,
) -> Result<Option<ExternalDatabase>, SavedSelectError> {
    let Some(row) = row else {
        return Ok(None);
    };
    context.check()?;
    validate(row)?;
    if !has_sources {
        return Err(SavedSelectError::InvalidMetadata);
    }

    // A NULL native path with a connection is serialized as an empty IN token
    // by the independent getter. The unmodified definition retains the NULL.
    // No parser interpolation: punctuation stays data rather than SQL clauses.
    let database = context.text(&[row.name1().unwrap_or("")])?;
    let connection = row
        .expression()
        .map(|value| context.text(&[value]))
        .transpose()?;
    context.check()?;

    Ok(Some(ExternalDatabase {
        database,
        connection_bracketed: connection.is_some(),
        connection,
    }))
}
