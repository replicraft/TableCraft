//! Normal DELETE is an outcome-aware shared-session consumer, not native editing.
use super::{
    DeleteArguments, Error,
    error::UnexpectedReceipt,
    report::{NamespaceFacts, Reports},
};
use crate::{
    commands::query::collation::{SelectedCollation, TextCollation},
    native,
};
use std::io::Write;
use tablecraft_access::editing::{offline::OfflineWriteSession, sql};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_expressions::comparison::Collation;
use tablecraft_io::journal::RecoveryOutcome;
use tablecraft_sql::ParsedQuery;

pub(super) fn prepare(
    args: &DeleteArguments,
    budget: &mut ResourceBudget,
) -> Result<(ParsedQuery, Option<SelectedCollation>), Error> {
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
    sql::local_delete_table(query.query()).map_err(Error::Shape)?;
    let collation = args
        .collation
        .map(|policy| TextCollation::from(policy).build())
        .transpose()
        .map_err(Error::Native)?;
    Ok((query, collation))
}

pub(super) fn run(args: DeleteArguments, writer: &mut dyn Write) -> Result<(), Error> {
    let _span = tracing::debug_span!("cli.query.journal_delete").entered();
    let mut budget = args.work.budget();
    let (query, collation) = prepare(&args, &mut budget)?;
    let session =
        OfflineWriteSession::open(&args.database, native::CATALOG_SCOPE, &mut budget, &|| {
            false
        })
        .map_err(Error::OfflineOpen)?;
    let collation = collation.as_ref().map(|policy| policy as &dyn Collation);
    let staged = session
        .plan_delete(query.query(), &mut budget, collation, &|| false)
        .map_err(Error::Offline)?;
    // Complete host output allocation precedes journal setup or target effects.
    let reports = Reports::delete(staged.matched_rows(), staged.deleted_rows(), &mut budget)
        .map_err(Error::Native)?;
    let (session, receipt) = staged
        .apply(&args.journal, &mut budget, &|| false)
        .map_err(Error::Offline)?;
    if receipt.outcome() != Some(RecoveryOutcome::Committed) {
        return Err(Error::UnexpectedReceipt(UnexpectedReceipt {
            session,
            receipt,
        }));
    }
    session.release().map_err(Error::Offline)?;
    reports.publish(
        RecoveryOutcome::Committed,
        NamespaceFacts::from_receipt(receipt),
        writer,
    )
}
