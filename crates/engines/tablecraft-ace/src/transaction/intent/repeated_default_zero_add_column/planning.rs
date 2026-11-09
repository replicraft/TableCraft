//! Original repeated declaration reconstruction over the retained V2 owner.
use super::{
    NativeRepeatedDefaultZeroAddColumnIntent as Intent,
    RepeatedDefaultZeroAddColumnIntentError as Error,
};
use crate::{
    CreatedDefaultZeroColumnRequired, NativeImage, RepeatedDefaultZeroAddColumnCandidate,
    header::check, transaction::qualified_recovery::equal_image,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    self, DurableStorage, JournalError, PreparedResizableImage, ResizableDurableStorage,
};

/// Owning repeated-source candidate plus original declaration; no AFTER constructor.
pub struct RecoverableRepeatedDefaultZeroAddColumnPlan {
    candidate: RepeatedDefaultZeroAddColumnCandidate,
    pub(super) intent: Intent,
}

impl NativeImage {
    /// Seal only the separately proved repeated source as TCAC0004/profile4.
    /// The declaration is explicitly nullable LONG, stored Required(false), zero.
    /// This never retries the frozen first-ADD constructor after a refusal.
    pub fn plan_recoverable_repeated_default_zero_int32_column(
        self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableRepeatedDefaultZeroAddColumnPlan, Error> {
        let _span = tracing::debug_span!("native.repeated_number.plan_recoverable").entered();
        let candidate = self.prepare_repeated_default_zero_int32_column_candidate(
            table, column, budget, cancelled,
        )?;
        let intent = Intent::capture(&candidate, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(RecoverableRepeatedDefaultZeroAddColumnPlan { candidate, intent })
    }
}

impl RecoverableRepeatedDefaultZeroAddColumnPlan {
    /// Data-only captured original request, not persistence permission.
    pub fn intent(&self) -> &Intent {
        &self.intent
    }

    /// Complete captured external-property source, including every historical row.
    pub fn before_bytes(&self) -> &[u8] {
        self.candidate.before_bytes()
    }

    /// Complete equal-length candidate; the profile forbids allocation/map edits.
    pub fn after_bytes(&self) -> &[u8] {
        self.candidate.after_bytes()
    }

    /// Exact source-resolved table spelling.
    pub fn table(&self) -> &str {
        self.candidate.table()
    }

    /// Exact original requested field spelling.
    pub fn column(&self) -> &str {
        self.candidate.column()
    }

    /// Explicit declaration shared by every request in this distinct profile.
    pub fn required_property(&self) -> CreatedDefaultZeroColumnRequired {
        self.intent.required_property()
    }

    /// Rebuild the entire historical source and candidate from fresh BEFORE.
    /// Reconstruction and complete image equality spend separate cumulative pools.
    pub fn requalify_with_budgets(
        self,
        fresh: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        let _span = tracing::debug_span!("native.repeated_number.requalify").entered();
        same_image(fresh.as_bytes(), self.before_bytes(), proof, cancelled)?;
        self.intent
            .verify_before(fresh.as_bytes(), proof, cancelled)?;
        let candidate = self.intent.reconstruct(fresh, native, cancelled)?;
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
        check(proof, cancelled)?;
        Ok(Self {
            candidate,
            intent: self.intent,
        })
    }

    /// Prepare the exact qualified images using V2, including equal-length images.
    /// Hosts retain exclusive resources and own durable namespace/finalization.
    /// No fixed V1 or first-ADD intent decoder is attempted after any failure.
    pub fn prepare<'a, D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedResizableImage<'a, D, J>, JournalError> {
        let _span = tracing::debug_span!("native.repeated_number.prepare_resizable").entered();
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
) -> Result<(), Error> {
    equal_image(actual, expected, budget, cancelled).map_err(|error| match error {
        crate::NativeRecoveryQualificationError::Inspect(error) => Error::Inspect(error),
        crate::NativeRecoveryQualificationError::ImageMismatch => Error::ImageMismatch,
        error => Error::Proof(error),
    })
}
