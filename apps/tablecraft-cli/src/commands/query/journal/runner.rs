use super::{
    Error, RecoveryArguments, UpdateArguments, error::FinalizationPhase, preflight, report::Reports,
};
use crate::native;
use std::io::Write;
use tablecraft_access::editing::sql;
use tablecraft_ace::{NativeImage, NativeInt32EditIntent, qualify_jet4_int32_recovery};
use tablecraft_expressions::comparison::Collation;
use tablecraft_io::journal::{
    RecoveryOutcome,
    filesystem::{OfflineJournalPair, OfflineTarget},
    inspect_v1,
};

pub(super) fn update(args: UpdateArguments, writer: &mut dyn Write) -> Result<(), Error> {
    let _span = tracing::debug_span!("cli.query.journal_update").entered();
    let mut budget = args.work.budget();
    let (query, collation) = preflight::prepare(&args, &mut budget)?;

    // Capture and plan against the actual retained exclusive descriptor. Never
    // capture under a shared lease and reopen the textual path for mutation.
    let mut target =
        OfflineTarget::open(&args.database, &mut budget, &|| false).map_err(Error::Io)?;
    let image = NativeImage::capture(&mut target, native::CATALOG_SCOPE, &mut budget, &|| false)
        .map_err(Error::Native)?;
    let plan = sql::plan_recoverable_update(
        image,
        query.query(),
        &mut budget,
        collation.as_ref().map(|policy| policy as &dyn Collation),
        &|| false,
    )
    .map_err(Error::Planning)?;

    let matched = plan.matched_rows();
    let plan = plan.into_backend();
    let reports =
        Reports::update(matched, plan.changed_rows(), &mut budget).map_err(Error::Native)?;
    let intent = plan
        .intent()
        .encode(&mut budget, &|| false)
        .map_err(Error::Intent)?;
    let mut pair =
        OfflineJournalPair::create(target, &args.journal, intent, &mut budget, &|| false)
            .map_err(Error::Setup)?;

    // The guard borrows the pair's devices only inside this lexical scope.
    // Every commit error retains the active bundle and requires recovery.
    {
        let (mut target, mut journal) = pair.storage().map_err(Error::Pair)?;
        let prepared = plan
            .prepare(&mut target, &mut journal, &mut budget, &|| false)
            .map_err(Error::Prepare)?;
        prepared
            .commit(&mut budget, &|| false)
            .map_err(Error::Commit)?;
    }

    finish(pair, RecoveryOutcome::Committed, reports, writer)
}

pub(super) fn recover(args: RecoveryArguments, writer: &mut dyn Write) -> Result<(), Error> {
    let _span = tracing::debug_span!("cli.query.recover_update").entered();
    let mut budget = args.work.budget();
    let target = OfflineTarget::open(&args.database, &mut budget, &|| false).map_err(Error::Io)?;
    let mut pair = OfflineJournalPair::open_existing(
        target,
        &args.journal,
        NativeInt32EditIntent::MAX_ENCODED_BYTES as u64,
        &mut budget,
        &|| false,
    )
    .map_err(Error::Setup)?;
    let intent = NativeInt32EditIntent::decode(pair.intent_bytes(), &mut budget, &|| false)
        .map_err(Error::RecoveryIntent)?;
    let reports = Reports::recovery(&mut budget).map_err(Error::RecoveryResources)?;

    let outcome = {
        let (mut target, mut journal) = pair.storage().map_err(Error::Pair)?;
        let checked = inspect_v1(&mut journal, &mut budget, &|| false).map_err(Error::Recovery)?;
        let qualified = qualify_jet4_int32_recovery(
            checked,
            intent,
            native::CATALOG_SCOPE,
            &mut budget,
            &|| false,
        )
        .map_err(Error::Qualification)?;
        qualified
            .recover(&mut target, &mut budget, &|| false)
            .map_err(Error::Recovery)?
    };

    finish(pair, outcome, reports, writer)
}

fn finish(
    mut pair: OfflineJournalPair,
    outcome: RecoveryOutcome,
    reports: Reports,
    writer: &mut dyn Write,
) -> Result<(), Error> {
    // The IO owner records the known outcome before its first fallible terminal
    // action. No budget/cancellation reset, SQL replay or cleanup retry follows.
    pair.retire(outcome).map_err(|error| Error::Finalization {
        outcome,
        phase: FinalizationPhase::Retirement,
        error,
    })?;
    pair.cleanup_retired()
        .map_err(|error| Error::Finalization {
            outcome,
            phase: FinalizationPhase::Cleanup,
            error,
        })?;
    pair.release()
        .map_err(|error| Error::Release { outcome, error })?;

    reports.publish(outcome, writer)
}
