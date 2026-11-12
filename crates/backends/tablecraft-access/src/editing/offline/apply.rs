//! Shared retained lifecycle; action-specific plans supply honest optional counts.
use super::plan::MutationPlan;
use super::{
    Cause, CompletionKind, Failure, KnownOutcome, OfflineWriteSession, Phase, QueryChange, Receipt,
    SchemaChange, SessionState,
};
use std::path::Path;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{RecoveryOutcome, filesystem::OfflineJournalPair};

pub(super) enum CompletionFacts {
    Rows {
        matched: Option<usize>,
        changed: Option<usize>,
    },
    Schema(Option<SchemaChange>),
    #[cfg(feature = "ace16-create")]
    PopulatedTable {
        column_count: usize,
        inserted_rows: usize,
    },
    Query(Option<QueryChange>),
    #[cfg(feature = "ace16-create")]
    DatabaseOptions(super::DatabaseOptionsChange),
}

pub(super) fn apply(
    mut session: OfflineWriteSession,
    plan: MutationPlan,
    facts: CompletionFacts,
    bundle: &Path,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(OfflineWriteSession, Receipt), Failure> {
    let _span = tracing::debug_span!("access.offline.apply").entered();
    let (matched, changed, schema, query, database_options) = match facts {
        CompletionFacts::Rows { matched, changed } => (matched, changed, None, None, None),
        #[cfg(feature = "ace16-create")]
        CompletionFacts::PopulatedTable {
            column_count,
            inserted_rows,
        } => (
            None,
            Some(inserted_rows),
            Some(SchemaChange::TableAdded { column_count }),
            None,
            None,
        ),
        CompletionFacts::Schema(schema) => (None, None, schema, None, None),
        CompletionFacts::Query(query) => (None, None, None, query, None),
        #[cfg(feature = "ace16-create")]
        CompletionFacts::DatabaseOptions(options) => (None, None, None, None, Some(options)),
    };
    let intent = match plan.encode_intent(budget, cancelled) {
        Ok(intent) => intent,
        Err(cause) => return Err(session.fail(Phase::Intent, cause)),
    };
    let explicit_intent_limit = match plan.explicit_intent_limit(intent.len()) {
        Ok(limit) => limit,
        Err(cause) => return Err(session.fail(Phase::Intent, cause)),
    };
    let setup = (|| {
        session.require_ready()?;
        let slot = session.slot_mut()?;
        let target = slot.target.take().ok_or(Cause::InvalidState)?;
        let created = match explicit_intent_limit {
            Some(limit) => OfflineJournalPair::create_with_intent_limit(
                target, bundle, intent, limit, budget, cancelled,
            ),
            None => OfflineJournalPair::create(target, bundle, intent, budget, cancelled),
        };
        match created {
            Ok(pair) => {
                slot.pair = Some(pair);
                slot.state = SessionState::RetainedJournal;
            }
            Err(failure) => {
                slot.setup = Some(failure);
                slot.state = SessionState::IncompleteSetup;
                return Err(Cause::Setup);
            }
        }
        Ok(())
    })();

    if let Err(cause) = setup {
        return Err(session.fail(Phase::Setup, cause));
    }
    let result = (|| {
        let slot = session.slot_mut()?;
        let pair = slot.pair.as_mut().ok_or(Cause::InvalidState)?;
        let (mut target, mut journal) = pair.resizable_storage().map_err(Cause::Pair)?;
        plan.commit(&mut target, &mut journal, budget, cancelled)?;
        Ok(())
    })();

    if let Err(cause) = result {
        let phase = if matches!(cause, Cause::Commit(_)) {
            Phase::Commit
        } else {
            Phase::Prepare
        };
        return Err(session.fail(phase, cause));
    }
    session.finish(
        Receipt {
            kind: CompletionKind::Applied,
            retirement: None,
            cleanup: None,
            known: KnownOutcome::Native(RecoveryOutcome::Committed),
            matched,
            changed,
            schema,
            query,
            database_options,
        },
        RecoveryOutcome::Committed,
        false,
    )
}

#[cfg(all(
    test,
    feature = "ace16-create",
    any(target_os = "macos", target_os = "linux", windows)
))]
mod tests;
