use super::plan::MutationPlan;
use super::{Cause, Failure, OfflineWriteSession, Phase, Receipt, SessionState};
use crate::editing::{journal::NativeJournalEditor, sql};
use std::path::Path;
use tablecraft_ace::{RecoverableNativeDeletePlan, RecoverableNativeEditPlan};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::comparison::Collation;
use tablecraft_sql::query::Query;

/// Owns a normal plan and retained target before any journal is created. Hosts
/// may allocate their reports from these counts before calling apply.
pub struct StagedUpdate {
    session: OfflineWriteSession,
    plan: MutationPlan,
    matched: usize,
    changed: usize,
}

impl OfflineWriteSession {
    /// Capture from the SAME retained descriptor and use the existing normal
    /// shared planner. No journal is created; native admission still applies. Capture
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
            let table = sql::local_update_table(query)
                .map_err(|error| Cause::Planning(sql::RecoverableError::Shape(error)))?;
            let slot = self.slot_mut()?;
            slot.current = None;
            slot.state = SessionState::RefreshRequired;
            let image = NativeJournalEditor::capture_image(
                slot.target.as_mut().ok_or(Cause::InvalidState)?,
                slot.scope,
                table,
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

impl OfflineWriteSession {
    /// Plan one atomic local multi-target UPDATE from the SAME retained target.
    /// Every expression reads the original snapshot; no journal exists until
    /// the returned StagedUpdate is explicitly applied. Failures retain the lease.
    pub fn plan_batch_update(
        mut self,
        query: &Query,
        budget: &mut ResourceBudget,
        collation: Option<&dyn Collation>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedUpdate, Failure> {
        let _span = tracing::debug_span!("access.offline.plan_batch_update").entered();
        let result = (|| {
            self.require_ready()?;
            let table = sql::local_batch_update_table(query, false)
                .map_err(|error| Cause::Planning(sql::RecoverableError::Shape(error)))?;
            let slot = self.slot_mut()?;
            slot.current = None;
            slot.state = SessionState::RefreshRequired;
            let image = NativeJournalEditor::capture_image(
                slot.target.as_mut().ok_or(Cause::InvalidState)?,
                slot.scope,
                table,
                budget,
                cancelled,
            )
            .map_err(Cause::Native)?;
            slot.state = SessionState::Ready;
            let plan =
                sql::plan_recoverable_batch_update(image, query, budget, collation, cancelled)
                    .map_err(Cause::Planning)?;
            let matched = plan.matched_rows();
            Ok((plan.into_backend(), matched))
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
        Self::from_mutation(session, MutationPlan::Update(plan), matched, changed)
    }

    pub(super) fn from_delete(
        session: OfflineWriteSession,
        plan: RecoverableNativeDeletePlan,
        matched: usize,
    ) -> Self {
        let changed = plan.deleted_rows();
        Self::from_mutation(session, MutationPlan::Delete(plan), matched, changed)
    }

    fn from_mutation(
        session: OfflineWriteSession,
        plan: MutationPlan,
        matched: usize,
        changed: usize,
    ) -> Self {
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
        super::apply::apply(
            self.session,
            self.plan,
            super::apply::CompletionFacts::Rows {
                matched: Some(self.matched),
                changed: Some(self.changed),
            },
            bundle,
            budget,
            cancelled,
        )
    }
}
