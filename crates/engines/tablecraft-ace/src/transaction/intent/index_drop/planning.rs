//! Sealed native candidate plus original request selects V3 explicitly.
use super::{IndexDropIntentError as Error, NativeIndexDropIntent as Intent};
use crate::{
    NativeImage, header::check, table::NativeIndexDropEffect,
    transaction::qualified_recovery::equal_image,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    self, DurableStorage, JournalError, PreparedResizableImage, ResizableDurableStorage,
};

/// Original index request and its independently reconstructed complete image pair.
/// Construction neither opens a journal nor acquires a durable namespace owner.
pub struct RecoverableNativeIndexDropPlan {
    effect: NativeIndexDropEffect,
    intent: Intent,
}

impl NativeImage {
    /// Qualify and independently prove one ordinary retained LONG index DROP,
    /// then bind its original names and native table/logical/physical identity
    /// to all BEFORE bytes. Alias survival and retirement are reconstructed;
    /// no SQL or decoded page list selects the effect.
    ///
    /// # Errors
    /// Refuses the native source/target, missing or constrained index, retirement scope,
    /// preservation failure, allocation, cancellation and explicit caller limits.
    pub fn plan_recoverable_retained_index_drop(
        self,
        table: &str,
        index: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeIndexDropPlan, Error> {
        let _span = tracing::debug_span!("native.index_drop.plan_recoverable").entered();
        let candidate = self.prepare_retained_index_drop(table, index, budget, cancelled)?;
        let (effect, intent) = Intent::capture(candidate.into_parts(), budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(RecoverableNativeIndexDropPlan { effect, intent })
    }
}

impl RecoverableNativeIndexDropPlan {
    /// Original request data; decoding it cannot reproduce native authority alone.
    pub fn intent(&self) -> &Intent {
        &self.intent
    }

    /// Complete original immutable native image.
    pub fn before_bytes(&self) -> &[u8] {
        self.effect.before_bytes()
    }

    /// Complete independently proved same-length replacement and allocation state.
    pub fn after_bytes(&self) -> &[u8] {
        self.effect.after_bytes()
    }

    /// Share the complete immutable AFTER with a readonly save/reopen witness.
    /// The already paid whole allocation is retained; no payload copy or mutable
    /// bytes are exposed, and this handle grants no native capture/write authority.
    pub fn after_shared_bytes(&self) -> bytes::Bytes {
        self.effect.after_shared_bytes()
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
        let Self { effect, intent } = self;
        equal_image(image.as_bytes(), effect.before_bytes(), proof, cancelled)?;
        let expected_after = effect.after_shared_bytes();
        drop(effect);
        let (effect, intent) = intent.reconstruct(image, native, cancelled)?;
        equal_image(effect.after_bytes(), &expected_after, proof, cancelled)?;
        Ok(Self { effect, intent })
    }

    /// Original qualified table name.
    pub fn table(&self) -> &str {
        self.intent.table()
    }

    /// Original requested index name.
    pub fn index_name(&self) -> &str {
        self.intent.index_name()
    }

    /// Reclaimed tree and dedicated map pages; not an affected-record count.
    pub fn retired_pages(&self) -> usize {
        self.effect.retired_pages()
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
        let _span = tracing::debug_span!("native.index_drop.prepare_v3").entered();
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
