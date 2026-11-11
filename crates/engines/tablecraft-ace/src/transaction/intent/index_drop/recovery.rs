//! Original index requests qualify the same checked V3 session before effects.
use super::{IndexDropIntentError as Error, NativeIndexDropIntent as Intent};
use crate::header::check;
#[cfg(feature = "ace16-create")]
use crate::{NativeImage, transaction::qualified_recovery::equal_image};
#[cfg(feature = "ace16-create")]
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    CheckedV3, DurableStorage, JournalError, RecoveryOutcome, ResizableDurableStorage,
    ResizableState,
};
use tablecraft_model::CatalogScope;

/// The same immutable V3 journal owner, qualified by fresh original-index reconstruction.
#[must_use = "recover explicitly or retain the checked native index journal"]
pub struct QualifiedNativeIndexDropRecovery<'a, J: DurableStorage> {
    checked: CheckedV3<'a, J>,
}

impl Intent {
    /// Verify complete BEFORE binding, requalify its native source in the supplied
    /// fresh scope, reconstruct the exact index and compare the complete AFTER.
    /// Decoded requests and checked framing separately grant no write authority.
    ///
    /// # Errors
    /// Rejects binding/identity/source/image mismatch, cancellation, allocation or
    /// caller limits before target effects. Without native creation this returns
    /// UnsupportedSource; no older decoder, SQL or protocol is attempted.
    pub fn qualify_retained_recovery<'a, J: DurableStorage>(
        self,
        checked: CheckedV3<'a, J>,
        scope: CatalogScope,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<QualifiedNativeIndexDropRecovery<'a, J>, Error> {
        let _span = tracing::debug_span!("native.index_drop.recovery_qualify").entered();
        check(budget, cancelled)?;
        #[cfg(feature = "ace16-create")]
        {
            self.verify_before(checked.before_bytes(), budget, cancelled)?;
            let image = NativeImage::capture_retained(
                &mut Cursor::new(checked.before_bytes()),
                scope,
                budget,
                cancelled,
            )?;
            let (effect, _original) = self.reconstruct(image, budget, cancelled)?;
            equal_image(
                effect.before_bytes(),
                checked.before_bytes(),
                budget,
                cancelled,
            )?;
            equal_image(
                effect.after_bytes(),
                checked.after_bytes(),
                budget,
                cancelled,
            )?;
            check(budget, cancelled)?;
            Ok(QualifiedNativeIndexDropRecovery { checked })
        }
        #[cfg(not(feature = "ace16-create"))]
        {
            let _ = (checked, scope);
            Err(Error::UnsupportedSource)
        }
    }
}

impl<J: DurableStorage> QualifiedNativeIndexDropRecovery<'_, J> {
    /// Exact framing state of the original checked session, not target health.
    pub fn state(&self) -> ResizableState {
        self.checked.state()
    }

    /// Recover only through the same qualified owner and explicit resizable target.
    ///
    /// # Errors
    /// Propagates target/journal I/O, barriers, binding, cancellation and caller limits.
    pub fn recover_retained<D: ResizableDurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span = tracing::debug_span!("native.index_drop.recovery_apply").entered();
        self.checked.recover(target, budget, cancelled)
    }

    /// Verify an already-settled target and durable barriers without replaying writes.
    ///
    /// # Errors
    /// Propagates settled-image mismatch, I/O/barriers, cancellation and caller limits.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span = tracing::debug_span!("native.index_drop.recovery_confirm").entered();
        self.checked.confirm_completed(target, budget, cancelled)
    }
}
