//! Original typed declaration reconstruction over the existing checked V2 owner.
use super::{FieldAdditionIntentError as Error, NativeFieldAdditionIntent as Intent};
use crate::{
    FieldAdditionCandidate, FieldAdditionTarget, NativeImage, header::check,
    transaction::qualified_recovery::equal_image,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    self, DurableStorage, JournalError, PreparedResizableImage, ResizableDurableStorage,
};

/// Owning typed candidate plus original declaration; no arbitrary AFTER constructor.
pub struct RecoverableFieldAdditionPlan {
    candidate: FieldAdditionCandidate,
    pub(super) intent: Intent,
}

impl NativeImage {
    /// Seal the separately proved typed candidate as TCAC0005/profile5.
    /// The original target fully binds width/compression and fixed field policies.
    /// This never invokes or retries a frozen legacy source constructor.
    pub fn plan_recoverable_created_field_addition(
        self,
        table: &str,
        column: &str,
        target: FieldAdditionTarget,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableFieldAdditionPlan, Error> {
        let _span = tracing::debug_span!("native.field_addition.plan_recoverable").entered();
        let candidate =
            self.prepare_created_add_field_candidate(table, column, target, budget, cancelled)?;
        let intent = Intent::capture(&candidate, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(RecoverableFieldAdditionPlan { candidate, intent })
    }
}

impl RecoverableFieldAdditionPlan {
    /// Data-only captured original request, not persistence permission.
    pub fn intent(&self) -> &Intent {
        &self.intent
    }

    /// Complete captured canonical or mixed-property source, including its history.
    pub fn before_bytes(&self) -> &[u8] {
        self.candidate.before_bytes()
    }

    /// Complete candidate for fitted or one-page rollover property storage.
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

    /// Exact semantic target bound to the actual sealed candidate.
    pub const fn target(&self) -> FieldAdditionTarget {
        self.candidate.target()
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
        let _span = tracing::debug_span!("native.field_addition.requalify").entered();
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
        let _span = tracing::debug_span!("native.field_addition.prepare_resizable").entered();
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
