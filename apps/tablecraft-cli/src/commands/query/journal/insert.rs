//! Normal INSERT is an outcome-aware shared-session consumer, not native editing.
use super::{Error, InsertArguments, error::UnexpectedReceipt, report::Reports};
use crate::native;
use std::io::Write;
use tablecraft_access::editing::{offline::OfflineWriteSession, sql};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_io::journal::RecoveryOutcome;
use tablecraft_sql::ParsedQuery;

pub(super) fn prepare(
    args: &InsertArguments,
    budget: &mut ResourceBudget,
) -> Result<ParsedQuery, Error> {
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
    sql::local_insert_table(query.query(), budget, &|| false).map_err(Error::Shape)?;
    Ok(query)
}

pub(super) fn run(args: InsertArguments, writer: &mut dyn Write) -> Result<(), Error> {
    let _span = tracing::debug_span!("cli.query.journal_insert").entered();
    let mut budget = args.work.budget();
    let query = prepare(&args, &mut budget)?;
    let session =
        OfflineWriteSession::open(&args.database, native::CATALOG_SCOPE, &mut budget, &|| {
            false
        })
        .map_err(Error::OfflineOpen)?;
    let staged = session
        .plan_insert(query.query(), &mut budget, &|| false)
        .map_err(Error::Offline)?;
    // Complete host output allocation precedes journal setup or target effects.
    let reports = Reports::insert(staged.inserted_rows(), &mut budget).map_err(Error::Native)?;
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
    reports.publish(RecoveryOutcome::Committed, writer)
}
