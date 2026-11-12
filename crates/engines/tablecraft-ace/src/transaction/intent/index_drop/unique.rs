//! Closed TCID0002 unique LONG original; shared inert framing is not source authority.
#[cfg(feature = "ace16-create")]
use super::{CapturePolicy, Identity};
use super::{IndexDropIntentError as Error, OriginalData as Data, codec, recovery};
#[cfg(feature = "ace16-create")]
use crate::{
    NativeImage,
    table::{NativeIndexDropEffect, NativeIndexDropParts},
    transaction::qualified_recovery::equal_image,
};
use tablecraft_core::limits::ResourceBudget;
#[cfg(feature = "ace16-create")]
use tablecraft_io::journal::{self, JournalError, PreparedResizableImage, ResizableDurableStorage};
use tablecraft_io::journal::{CheckedV3, DurableStorage};
use tablecraft_model::CatalogScope;

/// Original unique-secondary request. Decoding grants no source or persistence authority.
pub struct NativeUniqueLongIndexDropIntent {
    data: Data,
}

impl NativeUniqueLongIndexDropIntent {
    /// Maximum representation under the shared bounded name/identity envelope.
    pub const MAX_ENCODED_BYTES: usize = Data::MAX_ENCODED_BYTES;

    /// Decode only the exact unique family; old originals are not attempted.
    ///
    /// # Errors
    /// Refuses invalid family/profile, names, flags, identities or image lengths,
    /// allocation, cancellation and explicit caller resource exhaustion.
    pub fn decode(
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        codec::decode_unique(bytes, budget, cancelled).map(|data| Self { data })
    }

    /// Encode exact original data, never producer pages or mutation permission.
    ///
    /// # Errors
    /// Returns framing, allocation, cancellation and explicit resource failures.
    pub fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, Error> {
        codec::encode_unique(&self.data, budget, cancelled)
    }

    /// Verify every original byte before native reconstruction.
    ///
    /// # Errors
    /// Returns source-binding mismatch, native geometry, cancellation or limits.
    pub fn verify_before(
        &self,
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), Error> {
        self.data.verify_before(bytes, budget, cancelled)
    }

    /// Original table retained by this unique request.
    pub fn table(&self) -> &str {
        self.data.table()
    }

    /// Original index name retained by this unique request.
    pub fn index_name(&self) -> &str {
        self.data.index_name()
    }

    /// Original native table id retained by this unique request.
    pub fn native_table_id(&self) -> i32 {
        self.data.native_table_id()
    }

    /// Original definition page retained by this unique request.
    pub fn definition_page(&self) -> u32 {
        self.data.definition_page()
    }

    /// Original logical index number retained by this unique request.
    pub fn logical_index_number(&self) -> u32 {
        self.data.logical_index_number()
    }

    /// Original physical index number retained by this unique request.
    pub fn physical_index_number(&self) -> u32 {
        self.data.physical_index_number()
    }

    /// Original before length retained by this unique request.
    pub fn before_length(&self) -> u64 {
        self.data.before_length()
    }

    /// Reconstruct this exact unique operation before consuming the same V3 owner.
    ///
    /// # Errors
    /// Refuses source/identity/complete-image mismatch, unsupported native builds,
    /// allocation, cancellation and explicit resources before target effects.
    pub fn qualify_retained_recovery<'a, J: DurableStorage>(
        self,
        checked: CheckedV3<'a, J>,
        scope: CatalogScope,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<super::QualifiedNativeIndexDropRecovery<'a, J>, Error> {
        recovery::qualify(self, checked, scope, budget, cancelled)
    }

    #[cfg(feature = "ace16-create")]
    fn capture(
        parts: NativeIndexDropParts,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(NativeIndexDropEffect, Self), Error> {
        Data::capture(parts, CapturePolicy::UniqueLong, budget, cancelled)
            .map(|(effect, data)| (effect, Self { data }))
    }

    #[cfg(feature = "ace16-create")]
    fn reconstruct(
        self,
        image: NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(NativeIndexDropEffect, Self), Error> {
        self.verify_before(image.as_bytes(), budget, cancelled)?;
        let candidate = image.prepare_retained_unique_long_index_drop(
            self.table(),
            self.index_name(),
            budget,
            cancelled,
        )?;
        let NativeIndexDropParts {
            effect,
            table,
            index,
        } = candidate.into_parts();
        if !effect.is_unique_drop() || Identity::from_effect(&effect) != self.data.identity {
            return Err(Error::IdentityMismatch);
        }
        let data = Data {
            table,
            index,
            ..self.data
        };
        Ok((effect, Self { data }))
    }
}

impl recovery::Original for NativeUniqueLongIndexDropIntent {
    #[cfg(feature = "ace16-create")]
    fn verify_before(
        &self,
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), Error> {
        self.verify_before(bytes, budget, cancelled)
    }

    #[cfg(feature = "ace16-create")]
    fn effect(
        self,
        image: NativeImage,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeIndexDropEffect, Error> {
        self.reconstruct(image, budget, cancelled)
            .map(|(effect, _)| effect)
    }
}

#[cfg(feature = "ace16-create")]
/// A unique original and its independently proved immutable native effect.
pub struct RecoverableNativeUniqueLongIndexDropPlan {
    effect: NativeIndexDropEffect,
    intent: NativeUniqueLongIndexDropIntent,
}

#[cfg(feature = "ace16-create")]
impl NativeImage {
    /// Requalify the complete unique source and bind a separately recoverable original.
    ///
    /// # Errors
    /// Returns source, declaration, constraints, ownership, preservation, allocation,
    /// cancellation and explicit caller resource failures.
    pub fn plan_recoverable_retained_unique_long_index_drop(
        self,
        table: &str,
        index: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeUniqueLongIndexDropPlan, Error> {
        let candidate =
            self.prepare_retained_unique_long_index_drop(table, index, budget, cancelled)?;
        let (effect, intent) =
            NativeUniqueLongIndexDropIntent::capture(candidate.into_parts(), budget, cancelled)?;
        Ok(RecoverableNativeUniqueLongIndexDropPlan { effect, intent })
    }
}

#[cfg(feature = "ace16-create")]
impl RecoverableNativeUniqueLongIndexDropPlan {
    /// Exact original unique request, without mutable storage access.
    pub fn intent(&self) -> &NativeUniqueLongIndexDropIntent {
        &self.intent
    }

    /// Complete original immutable source image.
    pub fn before_bytes(&self) -> &[u8] {
        self.effect.before_bytes()
    }

    /// Complete independently proved replacement image.
    pub fn after_bytes(&self) -> &[u8] {
        self.effect.after_bytes()
    }

    /// Share the already paid immutable AFTER allocation with the save witness.
    pub fn after_shared_bytes(&self) -> bytes::Bytes {
        self.effect.after_shared_bytes()
    }

    /// Qualified table retained by this complete plan.
    pub fn table(&self) -> &str {
        self.intent.table()
    }

    /// Qualified index name retained by this complete plan.
    pub fn index_name(&self) -> &str {
        self.intent.index_name()
    }

    /// Qualified retired pages retained by this complete plan.
    pub fn retired_pages(&self) -> usize {
        self.effect.retired_pages()
    }

    /// Reconstruct against the actual exclusive source under separate work pools.
    ///
    /// # Errors
    /// Refuses source/identity/AFTER mismatch, native qualification, allocation,
    /// cancellation and explicit caller resource exhaustion.
    pub fn requalify_with_budgets(
        self,
        image: NativeImage,
        native: &mut ResourceBudget,
        proof: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        let Self { effect, intent } = self;
        equal_image(image.as_bytes(), effect.before_bytes(), proof, cancelled)?;
        let expected = effect.after_shared_bytes();
        drop(effect);
        let (effect, intent) = intent.reconstruct(image, native, cancelled)?;
        equal_image(effect.after_bytes(), &expected, proof, cancelled)?;
        Ok(Self { effect, intent })
    }

    /// Prepare only checked V3 on caller-owned exclusive resizable resources.
    ///
    /// # Errors
    /// Returns journal/source/framing, I/O/barrier, allocation, cancellation and
    /// cumulative resource failures without retrying another protocol.
    pub fn prepare_retained<'a, D: ResizableDurableStorage, J: DurableStorage>(
        self,
        target: &'a mut D,
        journal: &'a mut J,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PreparedResizableImage<'a, D, J>, JournalError> {
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

#[cfg(test)]
mod tests;
