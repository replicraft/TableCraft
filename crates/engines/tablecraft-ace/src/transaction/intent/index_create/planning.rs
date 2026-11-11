//! Sealed native candidate plus original request selects V3 explicitly.
use super::{IndexCreateIntentError as Error, NativeIndexCreateIntent as Intent};
use crate::{
    NativeImage, NativeLongIndexCandidate, SecondaryIndexCreate, header::check,
    transaction::qualified_recovery::equal_image,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    self, DurableStorage, JournalError, PreparedResizableImage, ResizableDurableStorage,
};

/// Original index request and its independently reconstructed complete image pair.
/// Construction neither opens a journal nor acquires a durable namespace owner.
pub struct RecoverableNativeIndexCreatePlan {
    candidate: NativeLongIndexCandidate,
    intent: Intent,
}

impl NativeImage {
    /// Qualify and independently prove one retained ordinary nullable LONG index,
    /// then bind its original names, direction and native identity to all BEFORE bytes.
    ///
    /// # Errors
    /// Refuses the native source/target, conflicting index name, actual capacity,
    /// preservation failure, allocation, cancellation and explicit caller limits.
    pub fn plan_recoverable_retained_long_index_creation(
        self,
        table: &str,
        request: &SecondaryIndexCreate<'_>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeIndexCreatePlan, Error> {
        let _span = tracing::debug_span!("native.index_create.plan_recoverable").entered();
        let candidate =
            self.prepare_retained_long_index_creation(table, request, budget, cancelled)?;
        let intent = Intent::capture(&candidate, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(RecoverableNativeIndexCreatePlan { candidate, intent })
    }
}

impl RecoverableNativeIndexCreatePlan {
    /// Original request data; decoding it cannot reproduce native authority alone.
    pub fn intent(&self) -> &Intent {
        &self.intent
    }
    /// Complete original immutable native image.
    pub fn before_bytes(&self) -> &[u8] {
        self.candidate.before_bytes()
    }
    /// Complete independently proved replacement, including EOF and allocation.
    pub fn after_bytes(&self) -> &[u8] {
        self.candidate.candidate_bytes()
    }
    /// Share the complete immutable AFTER with a readonly save/reopen witness.
    /// The already paid whole allocation is retained; no payload copy or mutable
    /// bytes are exposed, and this handle grants no native capture/write authority.
    pub fn after_shared_bytes(&self) -> bytes::Bytes {
        self.candidate.after_shared_bytes()
    }

    /// Rebuild the same original request against the actual exclusive source.
    /// Source construction uses its native pool; complete image comparisons use
    /// the separate cumulative proof pool. No SQL or default is reevaluated.
    ///
    /// # Errors
    /// Rejects original source/binding/identity or complete AFTER mismatch,
    /// native qualification failure, allocation, cancellation or caller limits.
    pub fn requalify_with_budgets(
        self,
        image: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        let Self { candidate, intent } = self;
        equal_image(image.as_bytes(), candidate.before_bytes(), proof, cancelled)?;
        let expected_after = candidate.after_shared_bytes();
        drop(candidate);
        let rebuilt = intent.reconstruct(image, native, cancelled)?;
        equal_image(rebuilt.candidate_bytes(), &expected_after, proof, cancelled)?;
        Ok(Self {
            candidate: rebuilt,
            intent,
        })
    }

    /// Original qualified table name.
    pub fn table(&self) -> &str {
        self.candidate.table()
    }
    /// Original selected native field name.
    pub fn column(&self) -> &str {
        self.candidate.column()
    }
    /// New index name.
    pub fn index_name(&self) -> &str {
        self.candidate.index_name()
    }
    /// Complete original live population, including NULL keys.
    pub fn indexed_rows(&self) -> usize {
        self.candidate.indexed_rows()
    }

    /// Prepare only the checked V3 expected-image protocol on explicit resizable storage.
    /// The caller owns offline resources and durable journal namespace lifecycle.
    ///
    /// # Errors
    /// Returns source mismatch, durable I/O/barrier, journal framing, allocation,
    /// cancellation or cumulative resource errors without alternate protocol retry.
    pub fn prepare_retained<'a, D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedResizableImage<'a, D, J>, JournalError> {
        let _span = tracing::debug_span!("native.index_create.prepare_v3").entered();
        journal::prepare_resizable_v3_expected(
            target,
            journal,
            self.before_bytes(),
            self.after_bytes(),
            budget,
            cancelled,
        )
    }
}
