//! Sealed explicit request and fresh complete reconstruction before V2 prepare.
use super::{FieldHistoryOptionsIntentError, NativeFieldHistoryDatabaseOptionsIntent};
use crate::{
    FieldHistoryDatabaseOptionsCandidate, NativeImage, header::check,
    transaction::qualified_recovery::equal_image,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    self, DurableStorage, JournalError, PreparedResizableImage, ResizableDurableStorage,
};

/// Exact checked OFF candidate and original source-bound request.
/// There is no constructor from a decoded request or arbitrary replacement bytes.
pub struct RecoverableFieldHistoryDatabaseOptionsPlan {
    candidate: FieldHistoryDatabaseOptionsCandidate,
    intent: NativeFieldHistoryDatabaseOptionsIntent,
}

impl NativeImage {
    /// Explicitly prepare OFF after field history and seal the distinct TCDO0002.
    /// Source and option admission remain the candidate's original closed language.
    pub fn plan_recoverable_disable_name_auto_correct_after_field_history(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableFieldHistoryDatabaseOptionsPlan, FieldHistoryOptionsIntentError> {
        let _span = tracing::debug_span!("native.database_options_field_history.plan_recoverable")
            .entered();
        let candidate =
            self.prepare_disable_name_auto_correct_after_field_history(budget, cancelled)?;
        let intent =
            NativeFieldHistoryDatabaseOptionsIntent::capture(&candidate, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(RecoverableFieldHistoryDatabaseOptionsPlan { candidate, intent })
    }
}

impl RecoverableFieldHistoryDatabaseOptionsPlan {
    /// Original explicit OFF request; serialized bytes are data, never permission.
    pub fn intent(&self) -> &NativeFieldHistoryDatabaseOptionsIntent {
        &self.intent
    }

    /// Complete immutable original that the target must still match.
    pub fn before_bytes(&self) -> &[u8] {
        self.candidate.before_bytes()
    }

    /// Complete newly reconstructed native candidate, not a completion receipt.
    pub fn after_bytes(&self) -> &[u8] {
        self.candidate.after_bytes()
    }

    /// Actual new database-property ROW in the existing shared column ledger.
    pub fn property_reference(&self) -> u32 {
        self.candidate.property_reference()
    }

    /// Reconstruct the complete original request with independent cumulative pools.
    /// No stored AFTER, physical patch or operation decoder supplies authority.
    pub fn requalify_with_budgets(
        self,
        fresh: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, FieldHistoryOptionsIntentError> {
        let _span =
            tracing::debug_span!("native.database_options_field_history.requalify").entered();
        same_image(fresh.as_bytes(), self.before_bytes(), proof, cancelled)?;
        self.intent
            .verify_before(fresh.as_bytes(), proof, cancelled)?;
        let candidate =
            fresh.prepare_disable_name_auto_correct_after_field_history(native, cancelled)?;
        same_image(
            candidate.before_bytes(),
            self.before_bytes(),
            proof,
            cancelled,
        )?;
        same_image(
            candidate.after_bytes(),
            self.after_bytes(),
            proof,
            cancelled,
        )?;
        if candidate.property_reference() != self.property_reference() {
            return Err(FieldHistoryOptionsIntentError::ImageMismatch);
        }
        check(proof, cancelled)?;
        Ok(Self {
            candidate,
            intent: self.intent,
        })
    }

    /// Prepare through the same checked V2 options lifecycle. This profile is
    /// currently equal-length; V2 use does not claim that it requires growth.
    /// The host retains exclusive target/journal association and durable namespace
    /// setup/cleanup. No fixed-length V1 or source-replacing fallback is attempted.
    pub fn prepare<'a, D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedResizableImage<'a, D, J>, JournalError> {
        let _span = tracing::debug_span!("native.database_options_field_history.prepare_resizable")
            .entered();
        journal::prepare_resizable_expected(
            target,
            journal,
            self.before_bytes(),
            self.after_bytes(),
            budget,
            cancelled,
        )
    }
}

pub(super) fn same_image(
    actual: &[u8],
    expected: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), FieldHistoryOptionsIntentError> {
    equal_image(actual, expected, budget, cancelled).map_err(|error| match error {
        crate::NativeRecoveryQualificationError::Inspect(error) => {
            FieldHistoryOptionsIntentError::Inspect(error)
        }
        crate::NativeRecoveryQualificationError::ImageMismatch => {
            FieldHistoryOptionsIntentError::ImageMismatch
        }
        error => FieldHistoryOptionsIntentError::Proof(error),
    })
}
