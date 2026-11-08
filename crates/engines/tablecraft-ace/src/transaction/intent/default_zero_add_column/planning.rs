//! Fresh native declaration reconstruction before a resizable checked journal.
use super::{DefaultZeroAddColumnIntentError as Error, NativeDefaultZeroAddColumnIntent as Intent};
use crate::{
    CreatedDefaultZeroAddColumnCandidate, CreatedDefaultZeroColumnRequired, NativeImage,
    header::check, transaction::qualified_recovery::equal_image,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    self, DurableStorage, JournalError, PreparedResizableImage, ResizableDurableStorage,
};

/// Owning native explicit-zero candidate plus its sealed original declaration.
/// There is no constructor from arbitrary AFTER bytes or decoded request data.
pub struct RecoverableDefaultZeroAddColumnPlan {
    candidate: CreatedDefaultZeroAddColumnCandidate,
    pub(super) intent: Intent,
}

impl NativeImage {
    /// Seal exact zero/default and Required-property presence as TCAC0003/profile3.
    /// Complete native source/program/property/allocation proof remains mandatory.
    pub fn plan_recoverable_created_add_default_zero_int32_column(
        self,
        table: &str,
        column: &str,
        required: CreatedDefaultZeroColumnRequired,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableDefaultZeroAddColumnPlan, Error> {
        let _span =
            tracing::debug_span!("native.default_zero_add_column.plan_recoverable").entered();
        let candidate = self.prepare_created_add_default_zero_int32_column_candidate(
            table, column, required, budget, cancelled,
        )?;
        let intent = Intent::capture(&candidate, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(RecoverableDefaultZeroAddColumnPlan { candidate, intent })
    }
}

impl RecoverableDefaultZeroAddColumnPlan {
    /// Data-only original request, never a serialized permission token.
    pub fn intent(&self) -> &Intent {
        &self.intent
    }

    /// Entire original source that the exclusive target must still match.
    pub fn before_bytes(&self) -> &[u8] {
        self.candidate.before_bytes()
    }

    /// Complete qualified candidate including catalog/LVAL/allocation changes.
    pub fn after_bytes(&self) -> &[u8] {
        self.candidate.after_bytes()
    }

    /// Original source-resolved table name.
    pub fn table(&self) -> &str {
        self.candidate.table()
    }

    /// Exact proposed column name.
    pub fn column(&self) -> &str {
        self.candidate.column()
    }

    /// Explicit absent versus stored-false Required declaration.
    pub fn required_property(&self) -> CreatedDefaultZeroColumnRequired {
        self.candidate.required_property()
    }

    /// Repeat native source qualification and complete image comparison with
    /// separate cumulative native and proof pools, never a decoder retry.
    pub fn requalify_with_budgets(
        self,
        fresh: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        let _span = tracing::debug_span!("native.default_zero_add_column.requalify").entered();
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
        if candidate.required_property() != self.required_property() {
            return Err(Error::ImageMismatch);
        }
        check(proof, cancelled)?;
        Ok(Self {
            candidate,
            intent: self.intent,
        })
    }

    /// Prepare only checked resizable V2 against the exact retained BEFORE.
    /// Hosts own exclusive association, durable namespace and cleanup. No fixed
    /// V1 or other action/source decoder is attempted after a refusal.
    pub fn prepare<'a, D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedResizableImage<'a, D, J>, JournalError> {
        let _span =
            tracing::debug_span!("native.default_zero_add_column.prepare_resizable").entered();
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
