//! Closed original-request dispatch for active and already settled journals.
use super::{
    NativeRecoveryQualificationError, NativeUpdateBody, NativeUpdateIntent, NativeUpdateProfile,
    QualifiedJet4Recovery, qualified_recovery::equal_image, qualify_jet4_int32_recovery,
};
use crate::{NativeImage, header::check};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{CheckedV1, DurableStorage, JournalError, RecoveryOutcome, V1State};
use tablecraft_model::CatalogScope;

enum Qualified<'a, J: DurableStorage> {
    Jet4(QualifiedJet4Recovery<'a, J>),
    BeforeChange(CheckedV1<'a, J>),
    Ordinary(CheckedV1<'a, J>),
}

/// The same checked journal after complete original-intent replanning.
/// Neither its serialized profile nor the existence of an AFTER image grants
/// recovery: both complete images have been compared with the rebuilt plan.
#[must_use = "recover explicitly or retain the journal for later recovery"]
pub struct QualifiedNativeRecovery<'a, J: DurableStorage> {
    qualified: Qualified<'a, J>,
}

impl<J: DurableStorage> QualifiedNativeRecovery<'_, J> {
    /// Framing state of this exact immutable journal session.
    pub fn state(&self) -> V1State {
        match &self.qualified {
            Qualified::Jet4(qualified) => qualified.state(),
            Qualified::BeforeChange(checked) | Qualified::Ordinary(checked) => checked.state(),
        }
    }

    /// Recover using the original checked session. No cleanup or subsequent
    /// capture can replace a successful known outcome with an unknown failure.
    pub fn recover<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span = tracing::debug_span!("native.update_recovery.apply").entered();
        match self.qualified {
            Qualified::Jet4(qualified) => qualified.recover(target, budget, cancelled),
            Qualified::BeforeChange(checked) | Qualified::Ordinary(checked) => {
                checked.recover(target, budget, cancelled)
            }
        }
    }

    /// Verify a complete already settled image and its barriers without writes.
    /// The outcome describes current bytes, not historical macro execution.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span = tracing::debug_span!("native.update_recovery.confirm_completed").entered();
        match self.qualified {
            Qualified::Jet4(qualified) => qualified.confirm_completed(target, budget, cancelled),
            Qualified::BeforeChange(checked) | Qualified::Ordinary(checked) => {
                checked.confirm_completed(target, budget, cancelled)
            }
        }
    }
}

/// Select exactly the record's closed validator, reconstructing its original
/// source and user requests. No error-based fallback or target write occurs.
pub fn qualify_native_update_recovery<'a, J: DurableStorage>(
    checked: CheckedV1<'a, J>,
    intent: NativeUpdateIntent,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<QualifiedNativeRecovery<'a, J>, NativeRecoveryQualificationError> {
    let _span = tracing::debug_span!("native.update_recovery.qualify").entered();
    let (profile, body) = intent.into_body();
    let qualified = match (profile, body) {
        (NativeUpdateProfile::Jet4Int32, NativeUpdateBody::Int32(original)) => Qualified::Jet4(
            qualify_jet4_int32_recovery(checked, original, scope, budget, cancelled)?,
        ),
        (NativeUpdateProfile::Ace16BeforeChangeInt32, NativeUpdateBody::Int32(original)) => {
            check(budget, cancelled)?;
            original.verify_before(checked.before_bytes(), budget, cancelled)?;
            let image = NativeImage::capture(
                &mut Cursor::new(checked.before_bytes()),
                scope,
                budget,
                cancelled,
            )?;
            let plan = image.plan_before_change_int32_updates(
                original.table(),
                original.column(),
                original.updates(),
                budget,
                cancelled,
            )?;
            equal_image(
                plan.before_bytes(),
                checked.before_bytes(),
                budget,
                cancelled,
            )?;
            equal_image(plan.after_bytes(), checked.after_bytes(), budget, cancelled)?;
            check(budget, cancelled)?;
            Qualified::BeforeChange(checked)
        }
        (NativeUpdateProfile::Ace16BeforeChangeFixed, NativeUpdateBody::Fixed(original)) => {
            check(budget, cancelled)?;
            original.verify_before(checked.before_bytes(), budget, cancelled)?;
            let image = NativeImage::capture(
                &mut Cursor::new(checked.before_bytes()),
                scope,
                budget,
                cancelled,
            )?;
            let groups = original.borrowed_groups(budget, cancelled)?;
            let plan = image.plan_before_change_fixed_updates(
                original.table(),
                &groups,
                budget,
                cancelled,
            )?;
            equal_image(
                plan.before_bytes(),
                checked.before_bytes(),
                budget,
                cancelled,
            )?;
            equal_image(plan.after_bytes(), checked.after_bytes(), budget, cancelled)?;
            check(budget, cancelled)?;
            Qualified::BeforeChange(checked)
        }
        (NativeUpdateProfile::Ace16OrdinaryFixed, NativeUpdateBody::Fixed(original)) => {
            check(budget, cancelled)?;
            original.verify_before(checked.before_bytes(), budget, cancelled)?;
            let image = NativeImage::capture(
                &mut Cursor::new(checked.before_bytes()),
                scope,
                budget,
                cancelled,
            )?;
            let groups = original.borrowed_groups(budget, cancelled)?;
            let plan =
                image.plan_ordinary_fixed_updates(original.table(), &groups, budget, cancelled)?;
            equal_image(
                plan.before_bytes(),
                checked.before_bytes(),
                budget,
                cancelled,
            )?;
            equal_image(plan.after_bytes(), checked.after_bytes(), budget, cancelled)?;
            check(budget, cancelled)?;
            Qualified::Ordinary(checked)
        }
        _ => return Err(super::NativeIntentError::UnsupportedProfile.into()),
    };
    Ok(QualifiedNativeRecovery { qualified })
}
