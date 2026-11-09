//! Rebuild both complete images before consuming the original checked V2 session.
use super::{
    FieldHistoryOptionsIntentError, NativeFieldHistoryDatabaseOptionsIntent, planning::same_image,
};
use crate::{NativeImage, header::check};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    CheckedV2, DurableStorage, JournalError, RecoveryOutcome, ResizableDurableStorage, V2State,
};
use tablecraft_model::CatalogScope;

/// The same checked resizable session after source-bound explicit option proof.
#[must_use = "recover explicitly or retain the qualified database-options journal"]
pub struct QualifiedFieldHistoryDatabaseOptionsRecovery<'a, J: DurableStorage> {
    checked: CheckedV2<'a, J>,
}

/// Validate original intent and both images using fresh complete native admission.
/// This function neither writes a target nor reinterprets another action family.
pub fn qualify_native_field_history_database_options_recovery<'a, J: DurableStorage>(
    checked: CheckedV2<'a, J>,
    intent: NativeFieldHistoryDatabaseOptionsIntent,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<QualifiedFieldHistoryDatabaseOptionsRecovery<'a, J>, FieldHistoryOptionsIntentError> {
    let _span =
        tracing::debug_span!("native.database_options_field_history.recovery_qualify").entered();
    intent.verify_before(checked.before_bytes(), budget, cancelled)?;
    let image = NativeImage::capture(
        &mut Cursor::new(checked.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let candidate =
        image.prepare_disable_name_auto_correct_after_field_history(budget, cancelled)?;
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
    Ok(QualifiedFieldHistoryDatabaseOptionsRecovery { checked })
}

impl<J: DurableStorage> QualifiedFieldHistoryDatabaseOptionsRecovery<'_, J> {
    /// Framing state established by this exact retained journal session.
    pub fn state(&self) -> V2State {
        self.checked.state()
    }

    /// Restore or commit through the same checked session and resizable target.
    pub fn recover<D: ResizableDurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.recover(target, budget, cancelled)
    }

    /// Check settled bytes and storage barriers without repair or action replay.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        self.checked.confirm_completed(target, budget, cancelled)
    }
}
