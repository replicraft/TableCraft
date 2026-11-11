//! Original-request reconstruction before the retained checked V1 gains authority.
use super::super::{FieldDropIntentError as Error, NativeFieldDropIntent as Intent};
use crate::{NativeImage, header::check, transaction::qualified_recovery::equal_image};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{CheckedV1, DurableStorage, JournalError, RecoveryOutcome, V1State};
use tablecraft_model::CatalogScope;

/// The same checked V1 session after complete native source/request/image proof.
#[must_use = "recover explicitly or retain this qualified saved-field DROP journal"]
pub struct QualifiedFieldDropRecovery<'a, J: DurableStorage> {
    checked: CheckedV1<'a, J>,
}

/// Reconstruct the real candidate from the complete BEFORE and original request.
/// A valid raw intent or independently readable stored AFTER grants no authority.
/// One cumulative pool covers binding, source replay, reconstruction and comparison.
pub fn qualify_native_field_drop_recovery<'a, J: DurableStorage>(
    checked: CheckedV1<'a, J>,
    intent: Intent,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<QualifiedFieldDropRecovery<'a, J>, Error> {
    let _span = tracing::debug_span!("native.field_drop_recovery.qualify").entered();
    check(budget, cancelled)?;
    if intent.is_retained() || intent.is_retained_typed() {
        return Err(Error::InvalidRecord);
    }
    intent.verify_before(checked.before_bytes(), budget, cancelled)?;
    let image = NativeImage::capture(
        &mut Cursor::new(checked.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let candidate = intent.reconstruct(image, budget, cancelled)?;
    equal_image(
        candidate.before_bytes(),
        checked.before_bytes(),
        budget,
        cancelled,
    )?;
    equal_image(
        candidate.after_bytes(),
        checked.after_bytes(),
        budget,
        cancelled,
    )?;
    check(budget, cancelled)?;
    Ok(QualifiedFieldDropRecovery { checked })
}

impl<J: DurableStorage> QualifiedFieldDropRecovery<'_, J> {
    /// Exact retained journal framing state, without recapture.
    pub fn state(&self) -> V1State {
        self.checked.state()
    }

    /// Recover the caller-bound target through this same qualified session.
    pub fn recover<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.recover(target, budget, cancelled)
    }

    /// Confirm complete settled current bytes without replaying mutation writes.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.confirm_completed(target, budget, cancelled)
    }
}
