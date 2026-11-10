use super::{AddTableIntentError, NativeAddTableIntent, codec::Profile};
use crate::{
    AddTableSourceProfile, NativeImage, NativeRecoveryQualificationError, header::check,
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
    Retained(CheckedV3<'a, J>),
}

impl<J> CheckedJournal<'_, J> {
    fn before_bytes(&self) -> &[u8] {
        match self {
            Self::Original(checked) => checked.before_bytes(),
            Self::Retained(checked) => checked.before_bytes(),
        }
    }

    fn after_bytes(&self) -> &[u8] {
        match self {
            Self::Original(checked) => checked.after_bytes(),
            Self::Retained(checked) => checked.after_bytes(),
        }
    }
}

/// The same immutable journal after source-bound add-table reconstruction.
#[must_use = "recover explicitly or retain the qualified add-table journal"]
pub struct QualifiedAddTableRecovery<'a, J: DurableStorage> {
    checked: CheckedJournal<'a, J>,
}

/// Requalify the actual populated BEFORE and rebuild the original declared AFTER before recovery.
/// The persisted intent and replacement image are data, never mutation authority.
pub fn qualify_native_add_table_recovery<'a, J: DurableStorage>(
    checked: CheckedV2<'a, J>,
    intent: NativeAddTableIntent,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<QualifiedAddTableRecovery<'a, J>, NativeRecoveryQualificationError> {
    let _span = tracing::debug_span!("native.add_table_recovery.qualify").entered();
    check(budget, cancelled)?;
    if intent.profile == Profile::Retained || intent.is_native() {
        return Err(AddTableIntentError::UnsupportedProfile.into());
    }
    qualify_checked(
        CheckedJournal::Original(checked),
        intent,
        scope,
        budget,
        cancelled,
    )
}

impl NativeAddTableIntent {
    /// Reconstruct only TCAT6 against this exact checked V3 journal.
    /// Other request families or V2 records cannot enter this recovery route.
    pub fn qualify_retained_recovery<'a, J: DurableStorage>(
        self,
        checked: CheckedV3<'a, J>,
        scope: CatalogScope,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<QualifiedAddTableRecovery<'a, J>, NativeRecoveryQualificationError> {
        let _span = tracing::debug_span!("native.retained_add_table_recovery.qualify").entered();
        check(budget, cancelled)?;
        if self.profile != Profile::Retained {
            return Err(AddTableIntentError::UnsupportedProfile.into());
        }
        qualify_checked(
            CheckedJournal::Retained(checked),
            self,
            scope,
            budget,
            cancelled,
        )
    }

    /// Reconstruct only TCAT7 through this exact checked V3 journal.
    /// A retained TCAT6 request cannot enter this native declaration route.
    pub fn qualify_native_recovery<'a, J: DurableStorage>(
        self,
        checked: CheckedV3<'a, J>,
        scope: CatalogScope,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<QualifiedAddTableRecovery<'a, J>, NativeRecoveryQualificationError> {
        check(budget, cancelled)?;
        if !self.is_native() {
            return Err(AddTableIntentError::UnsupportedProfile.into());
        }
        qualify_checked(
            CheckedJournal::Retained(checked),
            self,
            scope,
            budget,
            cancelled,
        )
    }
}

fn qualify_checked<'a, J: DurableStorage>(
    checked: CheckedJournal<'a, J>,
    intent: NativeAddTableIntent,
    scope: CatalogScope,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<QualifiedAddTableRecovery<'a, J>, NativeRecoveryQualificationError> {
    intent.verify_before(checked.before_bytes(), budget, cancelled)?;
    let mut input = Cursor::new(checked.before_bytes());
    let image = match &checked {
        CheckedJournal::Original(_) => NativeImage::capture(&mut input, scope, budget, cancelled)?,
        CheckedJournal::Retained(_) => {
            NativeImage::capture_retained(&mut input, scope, budget, cancelled)?
        }
    };
    let plan = match intent.profile {
        Profile::NativeMixed | Profile::NativeRetained => {
            let expected = intent.source_profile();
            let plan = image.plan_add_table_native(intent.definition, budget, cancelled)?;
            if plan.source_profile() != expected {
                return Err(AddTableIntentError::UnsupportedProfile.into());
            }
            plan
        }
        Profile::Retained => image.plan_add_table_retained(intent.definition, budget, cancelled)?,
        Profile::Mixed => {
            let plan = image.plan_add_table_mixed(intent.definition, budget, cancelled)?;
            if plan.source_profile() != AddTableSourceProfile::Mixed {
                return Err(AddTableIntentError::UnsupportedProfile.into());
            }
            plan
        }
        Profile::Legacy | Profile::Repeated | Profile::Described | Profile::RepeatedDescribed => {
            image.plan_add_table_legacy(intent.definition, budget, cancelled)?
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
    Ok(QualifiedAddTableRecovery { checked })
}

impl<J: DurableStorage> QualifiedAddTableRecovery<'_, J> {
    /// Observed framing state of this exact checked session.
    pub fn state(&self) -> ResizableState {
        match &self.checked {
            CheckedJournal::Original(checked) => checked.state(),
            CheckedJournal::Retained(checked) => checked.state(),
        }
    }

    /// Recover through the same checked session, without recapture or cleanup.
    pub fn recover<D: ResizableDurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span = tracing::debug_span!("native.add_table_recovery.apply").entered();
        match self.checked {
            CheckedJournal::Original(checked) => checked.recover(target, budget, cancelled),
            CheckedJournal::Retained(checked) => checked.recover(target, budget, cancelled),
        }
    }

    /// Verify a settled current image without repairing or replaying anything.
    pub fn confirm_completed<D: DurableStorage>(
        self,
        target: &mut D,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoveryOutcome, JournalError> {
        let _span = tracing::debug_span!("native.add_table_recovery.confirm_completed").entered();
        match self.checked {
            CheckedJournal::Original(checked) => {
                checked.confirm_completed(target, budget, cancelled)
            }
            CheckedJournal::Retained(checked) => {
                checked.confirm_completed(target, budget, cancelled)
            }
        }
    }
}
