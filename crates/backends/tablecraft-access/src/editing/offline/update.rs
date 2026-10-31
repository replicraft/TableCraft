use super::{
    Cause, CompletionKind, Failure, KnownOutcome, OfflineWriteSession, Phase, Receipt, SessionState,
};
use crate::editing::sql;
use std::path::Path;
use tablecraft_ace::{NativeImage, RecoverableNativeEditPlan};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::comparison::Collation;
use tablecraft_io::journal::{RecoveryOutcome, filesystem::OfflineJournalPair};
use tablecraft_sql::query::Query;

/// Owns a normal plan and retained target before any journal is created. Hosts
/// may allocate their reports from these counts before calling apply.
pub struct StagedUpdate {
    session: OfflineWriteSession,
    plan: RecoverableNativeEditPlan,
    matched: usize,
    changed: usize,
}

impl OfflineWriteSession {
    /// Capture from the SAME retained descriptor and use the existing normal
    /// shared planner. No journal is created. ACE remains unavailable. Capture
    /// failure retains the target in RefreshRequired with previous receipts intact.
    pub fn plan_update(
        mut self,
        query: &Query,
        budget: &mut ResourceBudget,
        collation: Option<&dyn Collation>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedUpdate, Failure> {
        let _span = tracing::debug_span!("access.offline.plan_update").entered();
        let result = (|| {
            self.require_ready()?;
            sql::local_update_table(query)
                .map_err(|error| Cause::Planning(sql::RecoverableError::Shape(error)))?;
            let slot = self.slot_mut()?;
            slot.current = None;
            slot.state = SessionState::RefreshRequired;
            let image = NativeImage::capture(
                slot.target.as_mut().ok_or(Cause::InvalidState)?,
                slot.scope,
                budget,
                cancelled,
            )
            .map_err(Cause::Native)?;
            slot.state = SessionState::Ready;
            let plan = sql::plan_recoverable_update(image, query, budget, collation, cancelled)
                .map_err(Cause::Planning)?;
            let matched = plan.matched_rows();
            let plan = plan.into_backend();
            Ok((plan, matched))
        })();

        match result {
            Ok((plan, matched)) => Ok(StagedUpdate::from_plan(self, plan, matched)),
            Err(cause) => Err(self.fail(Phase::Planning, cause)),
        }
    }
}

impl StagedUpdate {
    /// Shared internal constructor; callers cannot supply image bytes or invent
    /// changed counts. Both SQL and typed sources retain the normal native plan.
    pub(super) fn from_plan(
        session: OfflineWriteSession,
        plan: RecoverableNativeEditPlan,
        matched: usize,
    ) -> Self {
        let changed = plan.changed_rows();
        Self {
            session,
            plan,
            matched,
            changed,
        }
    }

    /// Original matched-row count available before mutation/report allocation.
    pub const fn matched_rows(&self) -> usize {
        self.matched
    }

    /// Qualified effective changes available before journal preparation.
    pub const fn changed_rows(&self) -> usize {
        self.changed
    }

    /// Explicitly abandon an unprepared plan without dropping the target lease.
    pub fn into_session(self) -> OfflineWriteSession {
        self.session
    }

    /// Create the explicit fresh bundle and commit the normal plan once.
    /// Failure owns the same resources; prepare/commit errors never auto-retry,
    /// roll back, delete a journal or restore normal access.
    pub fn apply(
        self,
        bundle: &Path,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(OfflineWriteSession, Receipt), Failure> {
        let _span = tracing::debug_span!("access.offline.apply").entered();
        let Self {
            mut session,
            plan,
            matched,
            changed,
        } = self;
        let intent = match plan.intent().encode(budget, cancelled) {
            Ok(intent) => intent,
            Err(error) => return Err(session.fail(Phase::Intent, Cause::Intent(error))),
        };
        let setup = (|| {
            session.require_ready()?;
            let slot = session.slot_mut()?;
            let target = slot.target.take().ok_or(Cause::InvalidState)?;
            match OfflineJournalPair::create(target, bundle, intent, budget, cancelled) {
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
            let (mut target, mut journal) = pair.storage().map_err(Cause::Pair)?;
            let prepared = plan
                .prepare(&mut target, &mut journal, budget, cancelled)
                .map_err(Cause::Prepare)?;
            prepared.commit(budget, cancelled).map_err(Cause::Commit)?;
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
                known: KnownOutcome::Native(RecoveryOutcome::Committed),
                matched: Some(matched),
                changed: Some(changed),
            },
            RecoveryOutcome::Committed,
            false,
        )
    }
}
