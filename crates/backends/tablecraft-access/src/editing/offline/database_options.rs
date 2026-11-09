//! Explicit source-bound OFF staging on the same retained native target owner.
pub(super) mod intent;
pub(super) mod plan;
pub(super) mod recovery;

use super::plan::MutationPlan;
use super::{
    Cause, DatabaseOptionsChange, Failure, OfflineWriteSession, Phase, Receipt, SessionState,
};
use plan::Plan;
use std::path::Path;
use tablecraft_ace::{
    NativeImage, RecoverableFieldHistoryDatabaseOptionsPlan, RecoverableNativeDatabaseOptionsPlan,
};
use tablecraft_core::limits::ResourceBudget;

/// A reviewed explicit OFF operation; no journal exists until consuming apply.
pub struct StagedDatabaseOptions {
    session: OfflineWriteSession,
    plan: Plan,
}

impl OfflineWriteSession {
    /// Reconstruct the original explicit OFF request on this retained target.
    /// Native admission and complete source/image proof spend separate pools.
    /// No missing metadata is interpreted as disabled and no creator default changes.
    pub fn stage_reviewed_disable_name_auto_correct_with_budgets(
        self,
        reviewed: RecoverableNativeDatabaseOptionsPlan,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedDatabaseOptions, Failure> {
        self.stage_database_options_plan(Plan::Creator(reviewed), native, proof, cancelled)
    }

    /// Reconstruct the distinct OFF-after-field-history original request.
    /// The caller selected this sealed plan before native qualification; failed
    /// creator qualification is never retried as this source language.
    pub fn stage_reviewed_field_history_disable_name_auto_correct_with_budgets(
        self,
        reviewed: RecoverableFieldHistoryDatabaseOptionsPlan,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedDatabaseOptions, Failure> {
        self.stage_database_options_plan(Plan::FieldHistory(reviewed), native, proof, cancelled)
    }

    fn stage_database_options_plan(
        mut self,
        reviewed: Plan,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedDatabaseOptions, Failure> {
        let _span =
            tracing::debug_span!("access.offline.stage_reviewed_database_options").entered();
        let result = (|| {
            self.require_ready()?;
            let slot = self.slot_mut()?;
            slot.current = None;
            slot.state = SessionState::RefreshRequired;
            let image = NativeImage::capture(
                slot.target.as_mut().ok_or(Cause::InvalidState)?,
                slot.scope,
                native,
                cancelled,
            )
            .map_err(Cause::Native)?;
            slot.state = SessionState::Ready;
            super::table::require_original(
                image.as_bytes(),
                reviewed.before_bytes(),
                proof,
                cancelled,
            )?;
            reviewed.requalify_with_budgets(image, native, proof, cancelled)
        })();
        match result {
            Ok(plan) => Ok(StagedDatabaseOptions {
                session: self,
                plan,
            }),
            Err(cause) => Err(self.fail(Phase::Planning, cause)),
        }
    }
}

impl StagedDatabaseOptions {
    /// Discard the staged candidate and retain the same exclusive target session.
    pub fn into_session(self) -> OfflineWriteSession {
        self.session
    }

    /// Apply explicit OFF through the actual target's checked resizable V2 pair.
    /// The distinct receipt invents neither row counts nor table/query changes.
    pub fn apply(
        self,
        bundle: &Path,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(OfflineWriteSession, Receipt), Failure> {
        super::apply::apply(
            self.session,
            MutationPlan::DatabaseOptions(self.plan),
            super::apply::CompletionFacts::DatabaseOptions(
                DatabaseOptionsChange::NameAutoCorrectDisabled,
            ),
            bundle,
            budget,
            cancelled,
        )
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
