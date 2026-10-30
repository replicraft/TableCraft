use crate::{
    EditError, InspectError, NativeImage, NativeInt32EditIntent, NativeIntentError, header::check,
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_io::journal::{CheckedV1, DurableStorage, JournalError, RecoveryOutcome, V1State};
use tablecraft_model::CatalogScope;
use thiserror::Error;

const COMPARE_BLOCK_BYTES: usize = 64 * 1024;

/// Failure before a checked journal gains normal Jet4 recovery capability.
/// Refusal changes no target bytes; an already-interrupted target still requires
/// recovery or diagnosis. Persisted intent and journal contents are not permission.
#[derive(Debug, Error)]
pub enum NativeRecoveryQualificationError {
    /// Bounded source capture, cancellation, allocation or work failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Original intent framing/profile/before-image binding is not qualified.
    #[error(transparent)]
    Intent(#[from] NativeIntentError),
    /// The original source/requests fail the existing ordinary Jet4 write gates.
    #[error(transparent)]
    Edit(#[from] EditError),
    /// A complete journal image differs from the independently rebuilt plan.
    #[error("journal images differ from the qualified original native edit")]
    ImageMismatch,
}

/// A checked V1 journal whose original intent reproduces the entire ordinary
/// Jet4 plan. Owns the same immutable journal session; no mutable image/cache or
/// replacement-journal API exists. This does not grant ACE saves or discovery.
#[must_use = "recover explicitly or retain the journal for later recovery"]
pub struct QualifiedJet4Recovery<'a, J: DurableStorage> {
    checked: CheckedV1<'a, J>,
}

impl<J: DurableStorage> QualifiedJet4Recovery<'_, J> {
    /// Framing state observed in this exact borrowed journal, not target health.
    pub fn state(&self) -> V1State {
        self.checked.state()
    }

    /// Apply existing checked V1 recovery to the caller's independently bound
    /// target. Success returns the known outcome directly, without a later
    /// capture/cleanup that could erase it. Failure requires explicit recovery;
    /// this method never deletes or reuses the retained journal.
    pub fn recover<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span = tracing::debug_span!("native.jet4_recovery.apply").entered();
        self.checked.recover(target, budget, cancelled)
    }
}

fn equal_image(
    actual: &[u8],
    original: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), NativeRecoveryQualificationError> {
    check(budget, cancelled)?;
    if actual.len() != original.len() {
        return Err(NativeRecoveryQualificationError::ImageMismatch);
    }
    for (actual, original) in actual
        .chunks(COMPARE_BLOCK_BYTES)
        .zip(original.chunks(COMPARE_BLOCK_BYTES))
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, actual.len() as u64)
            .map_err(InspectError::from)?;
        if actual != original {
            return Err(NativeRecoveryQualificationError::ImageMismatch);
        }
    }
    check(budget, cancelled)?;
    Ok(())
}

/// Rebuild the original typed request from the checked journal BEFORE image,
/// retaining all ordinary standard-capture/Jet4/constraint/preservation gates.
/// Both complete images must match before returning a recovery capability.
/// No target is accepted here: qualification cannot perform a recovery write.
/// Hosts still own exclusive offline access, object pairing, durable namespaces,
/// trusted lifecycle/no-replay policy, and subsequent cleanup.
pub fn qualify_jet4_int32_recovery<'a, J: DurableStorage>(
    checked: CheckedV1<'a, J>,
    intent: NativeInt32EditIntent,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<QualifiedJet4Recovery<'a, J>, NativeRecoveryQualificationError> {
    let _span = tracing::debug_span!("native.jet4_recovery.qualify").entered();
    check(budget, cancelled)?;
    intent.verify_before(checked.before_bytes(), budget, cancelled)?;

    let image = NativeImage::capture(
        &mut Cursor::new(checked.before_bytes()),
        scope,
        budget,
        cancelled,
    )?;
    let plan = image.plan_int32_updates(
        intent.table(),
        intent.column(),
        intent.updates(),
        budget,
        cancelled,
    )?;

    // Replanning, rather than a serialized profile label or producer cache, is
    // the authority. Keep CheckedV1's original bytes separate from this plan.
    equal_image(
        plan.before_bytes(),
        checked.before_bytes(),
        budget,
        cancelled,
    )?;
    equal_image(plan.after_bytes(), checked.after_bytes(), budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(QualifiedJet4Recovery { checked })
}
