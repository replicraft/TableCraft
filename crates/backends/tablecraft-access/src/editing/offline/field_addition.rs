//! Typed column reconstruction through the retained exclusive target owner.
use super::plan::MutationPlan;
use super::{Cause, Failure, OfflineWriteSession, Phase, Receipt, SchemaChange, SessionState};
use plan::Plan;
use std::path::Path;
use tablecraft_ace::{
    NativeImage, RecoverableDefaultZeroAddColumnPlan, RecoverableFieldAdditionPlan,
    RecoverableRepeatedDefaultZeroAddColumnPlan,
};
use tablecraft_core::limits::ResourceBudget;

/// A checked schema operation; journal setup begins only on consuming apply.
pub struct StagedFieldAddition {
    session: OfflineWriteSession,
    plan: Plan,
}

impl OfflineWriteSession {
    /// Reconstruct the original explicit-zero ADD-column request against this retained target.
    /// Native admission and complete source/image proof consume separate pools.
    /// Refusal retains this owner without creating a journal namespace.
    pub fn stage_reviewed_default_zero_add_column_with_budgets(
        self,
        reviewed: RecoverableDefaultZeroAddColumnPlan,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedFieldAddition, Failure> {
        let _span =
            tracing::debug_span!("access.offline.stage_reviewed_default_zero_add_column").entered();
        self.stage_field_addition(Plan::First(reviewed), native, proof, cancelled)
    }

    /// Reconstruct the distinct repeated StoredFalse/zero request against this retained target.
    pub fn stage_reviewed_repeated_default_zero_add_column_with_budgets(
        self,
        reviewed: RecoverableRepeatedDefaultZeroAddColumnPlan,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedFieldAddition, Failure> {
        let _span =
            tracing::debug_span!("access.offline.stage_reviewed_repeated_default_zero_add_column")
                .entered();
        self.stage_field_addition(Plan::Repeated(reviewed), native, proof, cancelled)
    }

    /// Reconstruct the exact typed declaration against this retained original.
    /// This route never retries a first/repeated Number language after refusal.
    pub fn stage_reviewed_field_addition_with_budgets(
        self,
        reviewed: RecoverableFieldAdditionPlan,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedFieldAddition, Failure> {
        let _span = tracing::debug_span!("access.offline.stage_reviewed_field_addition").entered();
        self.stage_field_addition(Plan::Typed(reviewed), native, proof, cancelled)
    }

    fn stage_field_addition(
        mut self,
        reviewed: Plan,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<StagedFieldAddition, Failure> {
        let result = (|| {
            self.require_ready()?;
            let slot = self.slot_mut()?;
            slot.current = None;
            slot.state = SessionState::RefreshRequired;
            let capture = if reviewed.is_retained() {
                NativeImage::capture_retained
            } else {
                NativeImage::capture
            };
            let image = capture(
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
            Ok(plan) => Ok(StagedFieldAddition {
                session: self,
                plan,
            }),
            Err(cause) => Err(self.fail(Phase::Planning, cause)),
        }
    }
}

impl StagedFieldAddition {
    /// Exact original table spelling retained by the checked request.
    pub fn table(&self) -> &str {
        self.plan.table()
    }

    /// Exact appended column spelling, never a replacement serialized schema.
    pub fn column(&self) -> &str {
        self.plan.column()
    }

    /// Abandon staging while retaining the same exclusive target capability.
    pub fn into_session(self) -> OfflineWriteSession {
        self.session
    }

    /// Commit the checked original request through its resizable V2 journal and lifecycle.
    /// Schema receipts carry no invented matched or changed record counts.
    pub fn apply(
        self,
        bundle: &Path,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(OfflineWriteSession, Receipt), Failure> {
        super::apply::apply(
            self.session,
            MutationPlan::FieldAddition(self.plan),
            super::apply::CompletionFacts::Schema(Some(SchemaChange::ColumnAdded)),
            bundle,
            budget,
            cancelled,
        )
    }
}

pub(super) mod plan;

mod after_off;

mod after_off_history;
mod current;

#[cfg(test)]
mod tests;
