//! Current fixed Number declaration reconstruction over the checked V2 owner.
mod retained;

use super::candidate::Candidate;
use super::record::{
    CurrentFieldAdditionIntentError as Error, NativeCurrentFieldAdditionIntent as Intent,
};
use crate::{
    FieldAdditionTarget, NativeImage, header::check, transaction::qualified_recovery::equal_image,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    self, DurableStorage, JournalError, PreparedResizableImage, ResizableDurableStorage,
};

/// Owning typed candidate plus original declaration; no arbitrary AFTER constructor.
pub struct RecoverableCurrentFieldAdditionPlan {
    candidate: Candidate,
    intent: Intent,
}

impl NativeImage {
    /// Seal the current metadata candidate as TCAC0008/profile8.
    /// Its language fixes nullable LONG, stored Required(false) and default0.
    /// Current metadata is requalified; frozen TCAC1–7 are never retried.
    pub fn plan_recoverable_current_number_field_addition(
        self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableCurrentFieldAdditionPlan, Error> {
        let _span =
            tracing::debug_span!("native.current_field_addition.plan_recoverable").entered();
        let candidate = Candidate::Number(
            self.prepare_current_number_field_addition(table, column, budget, cancelled)?,
        );
        let intent = Intent::capture(&candidate, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(RecoverableCurrentFieldAdditionPlan { candidate, intent })
    }
    /// Seal an explicit current nullable Short Text ADD as TCAC0009/profile9.
    /// Old records retain their original bytes and read as NULL for the new field.
    pub fn plan_recoverable_current_short_text_field_addition(
        self,
        table: &str,
        column: &str,
        max_utf16_units: usize,
        unicode_compression: bool,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableCurrentFieldAdditionPlan, Error> {
        let _span =
            tracing::debug_span!("native.current_short_text_field_addition.plan_recoverable")
                .entered();
        let candidate = Candidate::ShortText(self.prepare_current_short_text_field_addition(
            table,
            column,
            max_utf16_units,
            unicode_compression,
            budget,
            cancelled,
        )?);
        let intent = Intent::capture(&candidate, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(RecoverableCurrentFieldAdditionPlan { candidate, intent })
    }
}

impl RecoverableCurrentFieldAdditionPlan {
    /// Data-only captured original request, not persistence permission.
    pub fn intent(&self) -> &Intent {
        &self.intent
    }

    /// Complete current source after actual metadata and ownership qualification.
    pub fn before_bytes(&self) -> &[u8] {
        self.candidate.before_bytes()
    }

    /// Complete same-length candidate with the user body fitted after the DB row.
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
    pub fn target(&self) -> FieldAdditionTarget {
        self.candidate.target()
    }

    /// Requalify actual source ownership and the entire candidate from fresh BEFORE.
    /// Reconstruction and complete image equality spend separate cumulative pools.
    pub fn requalify_with_budgets(
        self,
        fresh: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        let _span = tracing::debug_span!("native.current_field_addition.requalify").entered();
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
        let _span =
            tracing::debug_span!("native.current_field_addition.prepare_resizable").entered();
        if self.intent.is_retained() {
            return Err(JournalError::ImageSize);
        }
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
