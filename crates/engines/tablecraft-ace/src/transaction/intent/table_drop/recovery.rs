//! Reconstruct the original table DROP before granting the retained V1 recovery path.
use super::{NativeTableDropIntent as Intent, TableDropIntentError as Error};
use crate::{NativeImage, header::check, transaction::qualified_recovery::equal_image};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{CheckedV1, DurableStorage, JournalError, RecoveryOutcome, V1State};
use tablecraft_model::CatalogScope;

/// The same checked V1 resource after complete native original-action proof.
#[must_use = "recover explicitly or retain the qualified table DROP journal"]
pub struct QualifiedTableDropRecovery<'a, J: DurableStorage> {
    checked: CheckedV1<'a, J>,
}

/// Qualify complete original/replacement bytes by rebuilding the frozen DROP.
/// An encoded intent or a readable replacement alone grants no authority.
///
/// # Errors
/// Returns an error for mismatched BEFORE/AFTER, unqualified native source or
/// action identity, cancellation or exhausted resources.
pub fn qualify_native_table_drop_recovery<'a, J: DurableStorage>(
    checked: CheckedV1<'a, J>,
    intent: Intent,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<QualifiedTableDropRecovery<'a, J>, Error> {
    let _span = tracing::debug_span!("native.table_drop_recovery.qualify").entered();
    check(budget, cancelled)?;
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
    Ok(QualifiedTableDropRecovery { checked })
}

impl<J: DurableStorage> QualifiedTableDropRecovery<'_, J> {
    /// Exact retained journal framing state, without recapturing another resource.
    pub fn state(&self) -> V1State {
        self.checked.state()
    }

    /// Recover the target through this same qualified journal resource.
    ///
    /// # Errors
    /// Returns an error for target drift, storage/cancellation or resource failure.
    pub fn recover<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.recover(target, budget, cancelled)
    }

    /// Confirm complete settled current bytes without replaying mutation writes.
    ///
    /// # Errors
    /// Returns an error for unsettled/different target bytes or storage/resource failure.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.confirm_completed(target, budget, cancelled)
    }
}
