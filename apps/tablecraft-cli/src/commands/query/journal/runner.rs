use super::{
    Error, RecoveryArguments, UpdateArguments, error::UnexpectedReceipt, preflight, report::Reports,
};
use crate::native;
use std::io::Write;
use tablecraft_access::editing::offline::OfflineWriteSession;
use tablecraft_expressions::comparison::Collation;
use tablecraft_io::journal::RecoveryOutcome;

pub(super) fn update(args: UpdateArguments, writer: &mut dyn Write) -> Result<(), Error> {
    let _span = tracing::debug_span!("cli.query.journal_update").entered();
    let mut budget = args.work.budget();
    let (query, collation) = preflight::prepare(&args, &mut budget)?;

    let session =
        OfflineWriteSession::open(&args.database, native::CATALOG_SCOPE, &mut budget, &|| {
            false
        })
        .map_err(Error::OfflineOpen)?;
    let staged = session
        .plan_update(
            query.query(),
            &mut budget,
            collation.as_ref().map(|policy| policy as &dyn Collation),
            &|| false,
        )
        .map_err(Error::Offline)?;

    // Allocate and format the complete host report before journal creation or
    // target mutation. The shared backend owns the actual save lifecycle.
    let reports = Reports::update(staged.matched_rows(), staged.changed_rows(), &mut budget)
        .map_err(Error::Native)?;
    let (session, receipt) = staged
        .apply(&args.journal, &mut budget, &|| false)
        .map_err(Error::Offline)?;
    debug_assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
    session.release().map_err(Error::Offline)?;
    reports.publish(RecoveryOutcome::Committed, writer)
}

pub(super) fn recover(args: RecoveryArguments, writer: &mut dyn Write) -> Result<(), Error> {
    let _span = tracing::debug_span!("cli.query.recover_update").entered();
    let mut budget = args.work.budget();
    let session =
        OfflineWriteSession::open(&args.database, native::CATALOG_SCOPE, &mut budget, &|| {
            false
        })
        .map_err(Error::OfflineOpen)?;
    let reports = Reports::recovery(&mut budget).map_err(Error::RecoveryResources)?;
    let (session, receipt) = session
        .recover_update(&args.journal, &mut budget, &|| false)
        .map_err(Error::Offline)?;
    let Some(outcome) = receipt.outcome() else {
        return Err(Error::UnexpectedReceipt(UnexpectedReceipt {
            session,
            receipt,
        }));
    };
    session.release().map_err(Error::Offline)?;
    reports.publish(outcome, writer)
}
