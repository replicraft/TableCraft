//! Same-session V2 recovery after exact original declaration reconstruction.
use super::{
    DefaultZeroAddColumnIntentError as Error, NativeDefaultZeroAddColumnIntent as Intent,
    planning::same_image,
};
use crate::{NativeImage, header::check};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    CheckedV2, DurableStorage, JournalError, RecoveryOutcome, ResizableDurableStorage, V2State,
};
use tablecraft_model::CatalogScope;

/// The original checked V2 session after complete native reconstruction.
#[must_use = "recover explicitly or retain the qualified explicit-zero journal"]
pub struct QualifiedDefaultZeroAddColumnRecovery<'a, J: DurableStorage> {
    checked: CheckedV2<'a, J>,
}

/// Reconstruct both complete images using the exact original Required/default
/// declaration. No write, allocation fallback or stored AFTER authority is granted.
/// Every digest, native read/rebuild and proof spends the caller's SAME cumulative pool.
pub fn qualify_native_default_zero_add_column_recovery<'a, J: DurableStorage>(
    checked: CheckedV2<'a, J>,
    intent: Intent,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<QualifiedDefaultZeroAddColumnRecovery<'a, J>, Error> {
    let _span = tracing::debug_span!("native.default_zero_add_column.recovery_qualify").entered();
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
    Ok(QualifiedDefaultZeroAddColumnRecovery { checked })
}

impl<J: DurableStorage> QualifiedDefaultZeroAddColumnRecovery<'_, J> {
    /// Framing state established by this exact retained journal.
    pub fn state(&self) -> V2State {
        self.checked.state()
    }

    /// Explicitly restore/retain complete bytes and length on the paired target.
    pub fn recover<D: ResizableDurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.recover(target, budget, cancelled)
    }

    /// Verify a settled target through storage barriers without repair writes.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.confirm_completed(target, budget, cancelled)
    }
}
