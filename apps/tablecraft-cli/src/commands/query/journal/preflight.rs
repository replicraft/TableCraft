use super::{Error, UpdateArguments};
use crate::commands::query::collation::{SelectedCollation, TextCollation};
use tablecraft_access::editing::sql;
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::{ParsedQuery, query::Statement};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Route {
    Single,
    Batch,
}

pub(super) fn prepare(
    args: &UpdateArguments,
    budget: &mut ResourceBudget,
) -> Result<(ParsedQuery, Option<SelectedCollation>, Route), Error> {
    budget
        .charge_allocation(args.sql.len() as u64, 1)
        .map_err(InspectError::from)
        .map_err(Error::Native)?;
    budget
        .charge(Resource::Steps, args.sql.len() as u64)
        .map_err(InspectError::from)
        .map_err(Error::Native)?;

    let query = tablecraft_sql::parse_query(&args.sql, tablecraft_sql::ParseLimits::default())
        .map_err(Error::Parse)?;
    let route = match &query.query().statement {
        Statement::Update(update) if update.assignments.len() == 1 => {
            sql::local_update_table(query.query()).map_err(Error::Shape)?;
            Route::Single
        }
        Statement::Update(_) => {
            sql::local_batch_update_table(query.query(), false).map_err(Error::Shape)?;
            Route::Batch
        }
        _ => return Err(Error::Shape(tablecraft_query::QueryError::Unsupported)),
    };
    let collation = args
        .collation
        .map(|policy| TextCollation::from(policy).build())
        .transpose()
        .map_err(Error::Native)?;

    Ok((query, collation, route))
}
