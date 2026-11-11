#[cfg(feature = "ace16-create")]
mod current_rows;
#[cfg(feature = "ace16-create")]
mod historical_fixed;
#[cfg(feature = "ace16-create")]
mod historical_forwarded_materialization;
#[cfg(feature = "ace16-create")]
mod historical_materialization;
#[cfg(feature = "ace16-create")]
mod historical_mixed_materialization;
#[cfg(feature = "ace16-create")]
mod historical_present;
#[cfg(feature = "ace16-create")]
mod historical_text;
mod requalify;
#[cfg(feature = "ace16-create")]
mod retained_boolean_rows;
#[cfg(feature = "ace16-create")]
mod retained_currency_rows;
#[cfg(feature = "ace16-create")]
mod retained_fixed_scalar_rows;
#[cfg(feature = "ace16-create")]
mod retained_opaque_rows;
#[cfg(feature = "ace16-create")]
mod retained_rows;
#[cfg(feature = "ace16-create")]
mod retained_wide_value_rows;

use super::{
    NativeFittedEditIntent, NativeFittedIntentGroup, NativeFixedEditIntent, NativeFixedIntentGroup,
    NativeInt32EditIntent, NativeIntentError, NativeUpdateIntent, NativeUpdateProfile,
};
use crate::{
    EditError, Int32Update, NativeEditPlan, NativeImage, PreparedNativeEdit, PreparedNativeUpdate,
    format::page::PAGE_BYTES,
    limits::MAX_IMAGE_BYTES,
    table::{FixedCarriers, OrdinaryVariablePlanKind},
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{DurableStorage, JournalError, ResizableDurableStorage};
use thiserror::Error;

/// Existing native qualification or additional intent-binding failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RecoverableEditError {
    /// The unchanged normal native planner refused the operation.
    #[error(transparent)]
    Edit(#[from] EditError),
    /// Original request metadata could not be validated/copied/bound.
    #[error(transparent)]
    Intent(#[from] NativeIntentError),
}

/// A qualified normal plan retaining its exact original resolved request intent.
/// No public constructor from parts or decoded request grants this authority.
pub struct RecoverableNativeEditPlan {
    plan: NativeEditPlan,
    intent: NativeUpdateIntent,
}

impl NativeImage {
    /// Plan the normal Jet4 or positive-program ACE16 Int32 route, then bind
    /// moved ORIGINAL user requests to its exact before image. The Jet4 path
    /// and TCNI0001 bytes retain their existing checks and resource charges.
    /// Intent failure occurs before any journal/target write and returns no plan.
    pub fn plan_recoverable_int32_updates(
        self,
        table: &str,
        column: &str,
        updates: Vec<Int32Update>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
        let _span = tracing::debug_span!("native.edit.plan_recoverable").entered();
        let ace = matches!(
            self.catalog().header().format(),
            crate::FileFormat::Ace2016 | crate::FileFormat::Ace2019
        );
        let plan = if ace {
            self.plan_before_change_int32_updates(table, column, &updates, budget, cancelled)?
        } else {
            self.plan_int32_updates(table, column, &updates, budget, cancelled)?
        };
        let intent = NativeInt32EditIntent::new(
            table,
            column,
            updates,
            plan.before_bytes(),
            budget,
            cancelled,
        )?;
        let profile = if ace {
            NativeUpdateProfile::Ace16BeforeChangeInt32
        } else {
            NativeUpdateProfile::Jet4Int32
        };
        let intent = NativeUpdateIntent::from_original(profile, intent);
        Ok(RecoverableNativeEditPlan { plan, intent })
    }

    /// Plan original fixed groups using positive native BeforeChange admission,
    /// then retain those same owned requests for recovery. No preview conversion.
    pub fn plan_recoverable_fixed_updates(
        self,
        table: &str,
        groups: Vec<NativeFixedIntentGroup>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
        let _span = tracing::debug_span!("native.fixed_edit.plan_recoverable").entered();
        let borrowed = super::fixed::borrow_groups(table, &groups, budget, cancelled)?;
        let plan = self.plan_before_change_fixed_updates(table, &borrowed, budget, cancelled)?;
        drop(borrowed);
        let original =
            NativeFixedEditIntent::new(table, groups, plan.before_bytes(), budget, cancelled)?;
        let intent = NativeUpdateIntent::from_fixed(original);
        Ok(RecoverableNativeEditPlan { plan, intent })
    }

    /// Plan ordinary fixed requests only after native complete-content admission.
    /// The original groups and exact BEFORE binding are retained under a distinct
    /// intent profile. No serialized absence flag or engineering conversion occurs.
    /// Recovery independently reconstructs closure and the complete native plan.
    pub fn plan_recoverable_ordinary_fixed_updates(
        self,
        table: &str,
        groups: Vec<NativeFixedIntentGroup>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
        let _span = tracing::debug_span!("native.ordinary_fixed_edit.plan_recoverable").entered();
        let borrowed = super::fixed::borrow_groups(table, &groups, budget, cancelled)?;
        let kind = self.ordinary_fixed_carriers(table, &borrowed, budget, cancelled)?;
        let plan = match kind {
            FixedCarriers::Legacy => {
                self.plan_ordinary_fixed_updates(table, &borrowed, budget, cancelled)?
            }
            FixedCarriers::Ieee => {
                self.plan_ordinary_ieee_updates(table, &borrowed, budget, cancelled)?
            }
            FixedCarriers::Scalar => {
                self.plan_ordinary_scalar_updates(table, &borrowed, budget, cancelled)?
            }
            FixedCarriers::Guid => {
                self.plan_ordinary_guid_updates(table, &borrowed, budget, cancelled)?
            }
        };
        drop(borrowed);
        let original =
            NativeFixedEditIntent::new(table, groups, plan.before_bytes(), budget, cancelled)?;
        let intent = match kind {
            FixedCarriers::Legacy => NativeUpdateIntent::from_ordinary_fixed(original),
            FixedCarriers::Ieee => NativeUpdateIntent::from_ordinary_ieee_fixed(original),
            FixedCarriers::Scalar => NativeUpdateIntent::from_ordinary_scalar_fixed(original),
            FixedCarriers::Guid => NativeUpdateIntent::from_ordinary_guid_fixed(original),
        };
        Ok(RecoverableNativeEditPlan { plan, intent })
    }

    /// Bind original fitting requests, then qualify the real ordinary native plan.
    /// The request record contains no save permission or precomputed final cells.
    pub fn plan_recoverable_ordinary_fitted_updates(
        self,
        table: &str,
        groups: Vec<NativeFittedIntentGroup>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
        let _span = tracing::debug_span!("native.ordinary_fitted_edit.plan_recoverable").entered();
        let original =
            NativeFittedEditIntent::new(table, groups, self.as_bytes(), budget, cancelled)?;
        let borrowed = original.borrowed_groups(budget, cancelled)?;
        let plan = self.plan_ordinary_fitted_updates(table, &borrowed, budget, cancelled)?;
        drop(borrowed);
        let intent = NativeUpdateIntent::from_fitted(original);
        Ok(RecoverableNativeEditPlan { plan, intent })
    }
}

impl NativeImage {
    /// Explicit record growth within already owned DATA capacity. This route
    /// never retries a failed fitting plan or grows the file. Original requests
    /// and full BEFORE binding select a distinct closed recovery validator.
    pub fn plan_recoverable_ordinary_owned_growth_updates(
        self,
        table: &str,
        groups: Vec<NativeFittedIntentGroup>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
        let _span = tracing::debug_span!("native.ordinary_owned_growth.plan_recoverable").entered();
        let original =
            NativeFittedEditIntent::new(table, groups, self.as_bytes(), budget, cancelled)?;
        let borrowed = original.borrowed_groups(budget, cancelled)?;
        let plan = self.plan_ordinary_owned_growth_updates(table, &borrowed, budget, cancelled)?;
        if plan.after_bytes().len() != plan.before_bytes().len() {
            return Err(EditError::Preservation.into());
        }
        drop(borrowed);
        let intent = NativeUpdateIntent::from_owned_growth(original);
        Ok(RecoverableNativeEditPlan { plan, intent })
    }
}

impl NativeImage {
    /// Bind original requests and qualify exactly one appended EOF DATA page.
    /// No final page, map claim or allocation permission enters the intent.
    pub fn plan_recoverable_ordinary_eof_updates(
        self,
        table: &str,
        groups: Vec<NativeFittedIntentGroup>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
        let _span = tracing::debug_span!("native.ordinary_eof.plan_recoverable").entered();
        let original =
            NativeFittedEditIntent::new(table, groups, self.as_bytes(), budget, cancelled)?;
        let borrowed = original.borrowed_groups(budget, cancelled)?;
        let plan = self.plan_ordinary_eof_updates(table, &borrowed, budget, cancelled)?;
        validate_eof_length(&plan)?;
        drop(borrowed);
        let intent = NativeUpdateIntent::from_eof_allocation(original);
        Ok(RecoverableNativeEditPlan { plan, intent })
    }
}

fn validate_eof_length(plan: &NativeEditPlan) -> Result<(), EditError> {
    let expected = plan
        .before_bytes()
        .len()
        .checked_add(PAGE_BYTES)
        .ok_or(EditError::Preservation)?;
    if plan.after_bytes().len() != expected || expected as u64 > MAX_IMAGE_BYTES {
        return Err(EditError::Preservation);
    }
    Ok(())
}

impl NativeImage {
    /// Choose fitting, owned-page growth or one supported EOF allocation from
    /// complete original requests and native capacity facts before any staging.
    /// A capacity decision never catches a failed physical planner. Explicit
    /// fitted-only and owned-only operations retain their existing contracts.
    pub fn plan_recoverable_ordinary_variable_updates(
        self,
        table: &str,
        groups: Vec<NativeFittedIntentGroup>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
        let _span = tracing::debug_span!("native.ordinary_variable.plan_recoverable").entered();
        let original =
            NativeFittedEditIntent::new(table, groups, self.as_bytes(), budget, cancelled)?;
        let borrowed = original.borrowed_groups(budget, cancelled)?;
        let kind = self.ordinary_variable_plan_kind(table, &borrowed, budget, cancelled)?;
        let plan = match &kind {
            OrdinaryVariablePlanKind::Fitted => {
                self.plan_ordinary_fitted_updates(table, &borrowed, budget, cancelled)?
            }
            OrdinaryVariablePlanKind::Owned => {
                self.plan_ordinary_owned_growth_updates(table, &borrowed, budget, cancelled)?
            }
            OrdinaryVariablePlanKind::AllocateEof => {
                self.plan_ordinary_eof_updates(table, &borrowed, budget, cancelled)?
            }
        };
        if matches!(&kind, OrdinaryVariablePlanKind::AllocateEof) {
            validate_eof_length(&plan)?;
        } else if plan.after_bytes().len() != plan.before_bytes().len() {
            return Err(EditError::Preservation.into());
        }
        drop(borrowed);
        let intent = match kind {
            OrdinaryVariablePlanKind::Fitted => NativeUpdateIntent::from_fitted(original),
            OrdinaryVariablePlanKind::Owned => NativeUpdateIntent::from_owned_growth(original),
            OrdinaryVariablePlanKind::AllocateEof => {
                NativeUpdateIntent::from_eof_allocation(original)
            }
        };
        Ok(RecoverableNativeEditPlan { plan, intent })
    }
}

impl RecoverableNativeEditPlan {
    /// Original immutable resolved requests and source binding.
    pub fn intent(&self) -> &NativeUpdateIntent {
        &self.intent
    }

    /// Complete qualified original bytes.
    pub fn before_bytes(&self) -> &[u8] {
        self.plan.before_bytes()
    }

    /// Complete qualified replacement bytes, not a durable-save receipt.
    pub fn after_bytes(&self) -> &[u8] {
        self.plan.after_bytes()
    }

    /// Effective changed rows; no-op requests remain in the intent separately.
    pub fn changed_rows(&self) -> usize {
        self.plan.changed_rows()
    }

    /// Delegate existing expected-image journal preparation. Hosts still own
    /// correct distinct pairing, durable namespace setup and retained resources.
    /// Dropping/interruption retains the journal; every commit error needs recovery.
    /// EOF allocation requires prepare_update; retained TCNI17–21 requires prepare_retained.
    /// Both are rejected here before effects.
    pub fn prepare<'a, D: DurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedNativeEdit<'a, D, J>, JournalError> {
        if matches!(
            self.intent.profile(),
            NativeUpdateProfile::Ace16OrdinaryEofAllocation
                | NativeUpdateProfile::Ace16RetainedRows
                | NativeUpdateProfile::Ace16RetainedOpaqueRows
                | NativeUpdateProfile::Ace16RetainedCurrencyRows
                | NativeUpdateProfile::Ace16RetainedBooleanRows
                | NativeUpdateProfile::Ace16RetainedFixedScalarRows
                | NativeUpdateProfile::Ace16RetainedWideValueRows
        ) {
            return Err(JournalError::ImageSize);
        }
        self.plan.prepare(target, journal, budget, cancelled)
    }

    /// Prepare only TCNI17–21 through the original retained V3 journal family.
    pub fn prepare_retained<'a, D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<crate::PreparedResizableNativeEdit<'a, D, J>, JournalError> {
        if !matches!(
            self.intent.profile(),
            NativeUpdateProfile::Ace16RetainedRows
                | NativeUpdateProfile::Ace16RetainedOpaqueRows
                | NativeUpdateProfile::Ace16RetainedCurrencyRows
                | NativeUpdateProfile::Ace16RetainedBooleanRows
                | NativeUpdateProfile::Ace16RetainedFixedScalarRows
                | NativeUpdateProfile::Ace16RetainedWideValueRows
        ) {
            return Err(JournalError::ImageSize);
        }
        self.plan
            .prepare_retained(target, journal, budget, cancelled)
    }

    /// Prepare the record-selected journal with an explicit resizable target.
    /// EOF allocation uses V2; existing profiles use the unchanged V1 path.
    /// Both branches retain the same target/journal resources and host duties.
    /// Retained TCNI17–21 requires prepare_retained and refuses here before effects.
    pub fn prepare_update<'a, D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedNativeUpdate<'a, D, J>, JournalError> {
        if matches!(
            self.intent.profile(),
            NativeUpdateProfile::Ace16RetainedRows
                | NativeUpdateProfile::Ace16RetainedOpaqueRows
                | NativeUpdateProfile::Ace16RetainedCurrencyRows
                | NativeUpdateProfile::Ace16RetainedBooleanRows
                | NativeUpdateProfile::Ace16RetainedFixedScalarRows
                | NativeUpdateProfile::Ace16RetainedWideValueRows
        ) {
            return Err(JournalError::ImageSize);
        }
        if self.intent.profile() == NativeUpdateProfile::Ace16OrdinaryEofAllocation {
            self.plan
                .prepare_resizable(target, journal, budget, cancelled)
                .map(PreparedNativeUpdate::Resizable)
        } else {
            self.plan
                .prepare(target, journal, budget, cancelled)
                .map(PreparedNativeUpdate::Fixed)
        }
    }
}

#[cfg(test)]
mod requalification;
