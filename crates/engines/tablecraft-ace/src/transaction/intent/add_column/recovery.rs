//! Reconstruct the original declaration before consuming the same checked journal.
use super::{AddColumnIntentError, NativeAddColumnIntent, planning::same_image};
use crate::{NativeImage, header::check};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{CheckedV1, DurableStorage, JournalError, RecoveryOutcome, V1State};
use tablecraft_model::CatalogScope;

/// Original checked session after full native ADD-column reconstruction.
#[must_use = "recover or retain the qualified add-column journal explicitly"]
pub struct QualifiedAddColumnRecovery<'a, J: DurableStorage> {
    checked: CheckedV1<'a, J>,
}

/// Check both complete images against a fresh reconstruction of the original request.
/// This function cannot write a target or turn stored patches into authority.
pub fn qualify_native_add_column_recovery<'a, J: DurableStorage>(
    checked: CheckedV1<'a, J>,
    intent: NativeAddColumnIntent,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<QualifiedAddColumnRecovery<'a, J>, AddColumnIntentError> {
    let _span = tracing::debug_span!("native.add_column.recovery_qualify").entered();
    intent.verify_before(checked.before_bytes(), budget, cancelled)?;
    let image = NativeImage::capture(
        &mut Cursor::new(checked.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let candidate = intent.reconstruct(image, budget, cancelled)?;
    same_image(
        candidate.before_bytes(),
        checked.before_bytes(),
        budget,
        cancelled,
    )?;
    same_image(
        candidate.after_bytes(),
        checked.after_bytes(),
        budget,
        cancelled,
    )?;
    check(budget, cancelled)?;
    Ok(QualifiedAddColumnRecovery { checked })
}

impl<J: DurableStorage> QualifiedAddColumnRecovery<'_, J> {
    /// State read from this exact checked journal session.
    pub fn state(&self) -> V1State {
        self.checked.state()
    }

    /// Consume the same checked session to repair the paired target explicitly.
    pub fn recover<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.recover(target, budget, cancelled)
    }

    /// Confirm a settled complete target with storage barriers and no repair writes.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.confirm_completed(target, budget, cancelled)
    }
}
