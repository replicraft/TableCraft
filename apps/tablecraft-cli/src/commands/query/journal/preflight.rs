use super::{Error, UpdateArguments};
use crate::commands::query::collation::{SelectedCollation, TextCollation};
use tablecraft_access::editing::sql;
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn prepare(
    args: &UpdateArguments,
    budget: &mut ResourceBudget,
) -> Result<(tablecraft_sql::ParsedQuery, Option<SelectedCollation>), Error> {
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
    sql::local_update_table(query.query()).map_err(Error::Shape)?;
    let collation = args
        .collation
        .map(|policy| TextCollation::from(policy).build())
        .transpose()
        .map_err(Error::Native)?;

    Ok((query, collation))
}
