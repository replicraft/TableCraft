//! Complete native rename reconstruction before consuming the same checked journal.
use super::NativeTableRenameIntent;
use crate::{
    NativeImage, NativeRecoveryQualificationError, header::check,
    transaction::qualified_recovery::equal_image,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{CheckedV1, DurableStorage, JournalError, RecoveryOutcome, V1State};
use tablecraft_model::CatalogScope;

/// The original immutable V1 session after source-bound schema reconstruction.
#[must_use = "recover explicitly or retain the qualified rename journal"]
pub struct QualifiedTableRenameRecovery<'a, J: DurableStorage> {
    checked: CheckedV1<'a, J>,
}

/// Prove both complete images and original native identity before recovery.
/// Neither the TCRN record nor the persisted AFTER confers mutation authority.
pub fn qualify_native_table_rename_recovery<'a, J: DurableStorage>(
    checked: CheckedV1<'a, J>,
    intent: NativeTableRenameIntent,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<QualifiedTableRenameRecovery<'a, J>, NativeRecoveryQualificationError> {
    let _span = tracing::debug_span!("native.table_rename_recovery.qualify").entered();
    check(budget, cancelled)?;
    intent.verify_before(checked.before_bytes(), budget, cancelled)?;
    let image = NativeImage::capture(
        &mut Cursor::new(checked.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let plan = intent.reconstruct(image, budget, cancelled)?;
    intent.verify_plan(&plan)?;
    equal_image(
        plan.before_bytes(),
        checked.before_bytes(),
        budget,
        cancelled,
    )?;
    equal_image(plan.after_bytes(), checked.after_bytes(), budget, cancelled)?;
    check(budget, cancelled)?;

    Ok(QualifiedTableRenameRecovery { checked })
}

impl<J: DurableStorage> QualifiedTableRenameRecovery<'_, J> {
    /// Framing state established by inspection of this exact retained V1 session.
    pub fn state(&self) -> V1State {
        self.checked.state()
    }

    /// Consume the same qualified journal session to recover the paired target.
    /// No intent is reinterpreted, journal recaptured or namespace cleaned up.
    pub fn recover<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.recover(target, budget, cancelled)
    }

    /// Confirm a complete settled current image without replay or repair writes.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.confirm_completed(target, budget, cancelled)
    }
}
