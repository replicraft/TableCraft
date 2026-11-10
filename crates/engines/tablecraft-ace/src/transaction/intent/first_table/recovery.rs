use super::{FirstTableIntentError, NativeFirstTableIntent};
use crate::{
    NativeImage, NativeRecoveryQualificationError, header::check,
    transaction::qualified_recovery::equal_image,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{
    CheckedV2, CheckedV3, DurableStorage, JournalError, RecoveryOutcome, ResizableDurableStorage,
    ResizableState,
};
use tablecraft_model::CatalogScope;

enum CheckedJournal<'a, J> {
    Original(CheckedV2<'a, J>),
    Native(CheckedV3<'a, J>),
}

impl<J> CheckedJournal<'_, J> {
    fn before_bytes(&self) -> &[u8] {
        match self {
            Self::Original(checked) => checked.before_bytes(),
            Self::Native(checked) => checked.before_bytes(),
        }
    }

    fn after_bytes(&self) -> &[u8] {
        match self {
            Self::Original(checked) => checked.after_bytes(),
            Self::Native(checked) => checked.after_bytes(),
        }
    }
}

/// The same immutable journal after source-bound first-table reconstruction.
#[must_use = "recover explicitly or retain the qualified first-table journal"]
pub struct QualifiedFirstTableRecovery<'a, J: DurableStorage> {
    checked: CheckedJournal<'a, J>,
}

/// Rebuild the canonical BEFORE and original declared AFTER before recovery.
/// The persisted intent and replacement image are data, never mutation authority.
pub fn qualify_native_first_table_recovery<'a, J: DurableStorage>(
    checked: CheckedV2<'a, J>,
    intent: NativeFirstTableIntent,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<QualifiedFirstTableRecovery<'a, J>, NativeRecoveryQualificationError> {
    let _span = tracing::debug_span!("native.first_table_recovery.qualify").entered();
    check(budget, cancelled)?;
    if intent.is_native() {
        return Err(FirstTableIntentError::UnsupportedProfile.into());
    }
    qualify_checked(
        CheckedJournal::Original(checked),
        intent,
        scope,
        budget,
        cancelled,
    )
}

impl NativeFirstTableIntent {
    /// Reconstruct only TCNT7 through this exact checked V3 journal.
    pub fn qualify_native_recovery<'a, J: DurableStorage>(
        self,
        checked: CheckedV3<'a, J>,
        scope: CatalogScope,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<QualifiedFirstTableRecovery<'a, J>, NativeRecoveryQualificationError> {
        let _span = tracing::debug_span!("native.first_table_recovery.qualify_native").entered();
        check(budget, cancelled)?;
        if !self.is_native() {
            return Err(FirstTableIntentError::UnsupportedProfile.into());
        }
        qualify_checked(
            CheckedJournal::Native(checked),
            self,
            scope,
            budget,
            cancelled,
        )
    }
}

fn qualify_checked<'a, J: DurableStorage>(
    checked: CheckedJournal<'a, J>,
    intent: NativeFirstTableIntent,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<QualifiedFirstTableRecovery<'a, J>, NativeRecoveryQualificationError> {
    intent.verify_before(checked.before_bytes(), budget, cancelled)?;
    let mut input = Cursor::new(checked.before_bytes());
    let image = match &checked {
        CheckedJournal::Original(_) => NativeImage::capture(&mut input, scope, budget, cancelled)?,
        CheckedJournal::Native(_) => {
            NativeImage::capture_retained(&mut input, scope, budget, cancelled)?
        }
    };
    let plan = match &checked {
        CheckedJournal::Original(_) => {
            image.plan_first_table(intent.definition, budget, cancelled)?
        }
        CheckedJournal::Native(_) => {
            image.plan_first_table_native(intent.definition, budget, cancelled)?
        }
    };
    equal_image(
        plan.before_bytes(),
        checked.before_bytes(),
        budget,
        cancelled,
    )?;
    equal_image(plan.after_bytes(), checked.after_bytes(), budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(QualifiedFirstTableRecovery { checked })
}

impl<J: DurableStorage> QualifiedFirstTableRecovery<'_, J> {
    /// Observed framing state of this exact checked session.
    pub fn state(&self) -> ResizableState {
        match &self.checked {
            CheckedJournal::Original(checked) => checked.state(),
            CheckedJournal::Native(checked) => checked.state(),
        }
    }

    /// Recover through the same checked session, without recapture or cleanup.
    pub fn recover<D: ResizableDurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span = tracing::debug_span!("native.first_table_recovery.apply").entered();
        match self.checked {
            CheckedJournal::Original(checked) => checked.recover(target, budget, cancelled),
            CheckedJournal::Native(checked) => checked.recover(target, budget, cancelled),
        }
    }

    /// Verify a settled current image without repairing or replaying anything.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span = tracing::debug_span!("native.first_table_recovery.confirm_completed").entered();
        match self.checked {
            CheckedJournal::Original(checked) => {
                checked.confirm_completed(target, budget, cancelled)
            }
            CheckedJournal::Native(checked) => checked.confirm_completed(target, budget, cancelled),
        }
    }
}
