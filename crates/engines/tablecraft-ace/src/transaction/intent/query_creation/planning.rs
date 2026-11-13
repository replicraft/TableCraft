//! Sealed QueryDef effect plus semantic original selects only checked V3.
use super::{NativeQueryCreationIntent as Intent, QueryCreationIntentError as Error};
use crate::{
    NativeImage, NativeQueryStorageChange, NativeRecoveryQualificationError,
    QueryCreationDefinition, header::check, queries::creation::effect::NativeQueryCreationEffect,
    transaction::qualified_recovery::equal_image,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    self, DurableStorage, JournalError, PreparedResizableImage, ResizableDurableStorage,
};

/// Original request and independently proved complete native image pair.
/// There is no constructor from decoded data or arbitrary replacement bytes.
pub struct RecoverableNativeQueryCreationPlan {
    effect: NativeQueryCreationEffect,
    intent: Intent,
}

impl NativeImage {
    /// Construct and prove a new constant QueryDef, then bind its complete source.
    /// The explicit captured DATE is retained without consulting a clock.
    ///
    /// # Errors
    /// Refuses native declaration/source/identity, preservation, cancellation,
    /// allocation and cumulative caller limits before durable resources are used.
    pub fn plan_recoverable_query_creation(
        self,
        definition: QueryCreationDefinition,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeQueryCreationPlan, Error> {
        let _span = tracing::debug_span!("native.query_creation.plan_recoverable").entered();
        let candidate = self.prepare_query_creation(definition, budget, cancelled)?;
        let (effect, intent) = Intent::capture(candidate.into_creation_parts(), budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(RecoverableNativeQueryCreationPlan { effect, intent })
    }
}

impl RecoverableNativeQueryCreationPlan {
    /// Exact technical storage effect from the independently proved native plan.
    pub const fn storage_change(&self) -> NativeQueryStorageChange {
        self.effect.storage_change()
    }

    /// Semantic original data; encoding grants no source or recovery authority.
    pub fn intent(&self) -> &Intent {
        &self.intent
    }

    /// Complete immutable original image.
    pub fn before_bytes(&self) -> &[u8] {
        self.effect.before_bytes()
    }

    /// Complete independently proved new-object image.
    pub fn after_bytes(&self) -> &[u8] {
        self.effect.after_bytes()
    }

    /// Share the already paid immutable AFTER for an independent readonly witness.
    /// No mutable bytes or native operation capability are exposed.
    pub fn after_shared_bytes(&self) -> bytes::Bytes {
        self.effect.after_shared_bytes()
    }

    /// New native catalog identity, not a page or scoped application object ID.
    pub const fn native_query_id(&self) -> i32 {
        self.intent.native_query_id()
    }

    /// Reconstruct the same original against the actual fresh exclusive source.
    /// Native construction and complete byte comparisons use separate pools.
    ///
    /// # Errors
    /// Refuses stale source/binding/identity, changed AFTER, native reconstruction,
    /// allocation, cancellation or independent caller resource exhaustion.
    pub fn requalify_with_budgets(
        self,
        image: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        let Self { effect, intent } = self;
        equal_image(image.as_bytes(), effect.before_bytes(), proof, cancelled).map_err(
            |error| match error {
                NativeRecoveryQualificationError::ImageMismatch => Error::BeforeMismatch,
                other => Error::Proof(other),
            },
        )?;
        let expected_after = effect.after_shared_bytes();
        drop(effect);
        let (effect, intent) = intent.reconstruct(image, native, cancelled)?;
        equal_image(effect.after_bytes(), &expected_after, proof, cancelled)?;
        check(proof, cancelled)?;
        Ok(Self { effect, intent })
    }

    /// Prepare only the checked V3 expected-image protocol on caller-owned storage.
    /// The caller durably pairs encoded intent with this journal using the real
    /// opaque-intent bundle and owns exclusive namespace setup/cleanup. The raw
    /// V3 image protocol alone does not persist the native semantic original.
    ///
    /// # Errors
    /// Returns target mismatch, durable I/O/barriers, allocation, cancellation
    /// and cumulative resource failures without retrying another journal family.
    pub fn prepare_retained<'a, D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedResizableImage<'a, D, J>, JournalError> {
        let _span = tracing::debug_span!("native.query_creation.prepare_v3").entered();
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
