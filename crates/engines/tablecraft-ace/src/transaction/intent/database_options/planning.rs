//! Sealed explicit request and fresh complete reconstruction before V2 prepare.
use super::{DatabaseOptionsIntentError, NativeDatabaseOptionsIntent};
use crate::{
    NativeDatabaseOptionsPlan, NativeImage, header::check,
    transaction::qualified_recovery::equal_image,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    self, DurableStorage, JournalError, PreparedResizableImage, ResizableDurableStorage,
};

/// Exact checked OFF candidate and original source-bound request.
/// There is no constructor from a decoded request or arbitrary replacement bytes.
pub struct RecoverableNativeDatabaseOptionsPlan {
    candidate: NativeDatabaseOptionsPlan,
    intent: NativeDatabaseOptionsIntent,
}

impl NativeImage {
    /// Explicitly prepare disabled Track/Perform options and seal TCDO0001.
    /// Source and option admission remain the candidate's original closed language.
    pub fn plan_recoverable_disable_name_auto_correct(
        self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeDatabaseOptionsPlan, DatabaseOptionsIntentError> {
        let _span = tracing::debug_span!("native.database_options.plan_recoverable").entered();
        let candidate = self.plan_disable_name_auto_correct(budget, cancelled)?;
        let intent = NativeDatabaseOptionsIntent::capture(&candidate, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(RecoverableNativeDatabaseOptionsPlan { candidate, intent })
    }
}

impl RecoverableNativeDatabaseOptionsPlan {
    /// Original explicit OFF request; serialized bytes are data, never permission.
    pub fn intent(&self) -> &NativeDatabaseOptionsIntent {
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

    /// Fresh native storage fact for the new single-row external property body.
    pub fn property_page(&self) -> u32 {
        self.candidate.property_page()
    }

    /// Reconstruct the complete original request with independent cumulative pools.
    /// No stored AFTER, physical patch or operation decoder supplies authority.
    pub fn requalify_with_budgets(
        self,
        fresh: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, DatabaseOptionsIntentError> {
        let _span = tracing::debug_span!("native.database_options.requalify").entered();
        same_image(fresh.as_bytes(), self.before_bytes(), proof, cancelled)?;
        self.intent
            .verify_before(fresh.as_bytes(), proof, cancelled)?;
        let candidate = fresh.plan_disable_name_auto_correct(native, cancelled)?;
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
        if candidate.property_page() != self.property_page() {
            return Err(DatabaseOptionsIntentError::ImageMismatch);
        }
        check(proof, cancelled)?;
        Ok(Self {
            candidate,
            intent: self.intent,
        })
    }

    /// Prepare the length-changing V2 journal through an explicit resizable target.
    /// The host retains exclusive target/journal association and durable namespace
    /// setup/cleanup. No fixed-length V1 or source-replacing fallback is attempted.
    pub fn prepare<'a, D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedResizableImage<'a, D, J>, JournalError> {
        let _span = tracing::debug_span!("native.database_options.prepare_resizable").entered();
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
) -> Result<(), DatabaseOptionsIntentError> {
    equal_image(actual, expected, budget, cancelled).map_err(|error| match error {
        crate::NativeRecoveryQualificationError::Inspect(error) => {
            DatabaseOptionsIntentError::Inspect(error)
        }
        crate::NativeRecoveryQualificationError::ImageMismatch => {
            DatabaseOptionsIntentError::ImageMismatch
        }
        error => DatabaseOptionsIntentError::Proof(error),
    })
}
