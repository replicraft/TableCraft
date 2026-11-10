#[cfg(feature = "journal")]
mod deleting;

#[cfg(feature = "journal")]
use super::journal::JournalRoute;
use super::{NativeEditError, snapshot};
#[cfg(feature = "journal")]
use tablecraft_ace::{
    EditError, NativeFittedIntentGroup, NativeFixedIntentGroup, RecoverableEditError,
    RecoverableNativeEditPlan,
};
use tablecraft_ace::{FileFormat, InspectError, Int32Update, NativeEditPlan, NativeImage};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::{CellUpdate, EditableTable, Snapshot, UpdateProfile};

/// Actual scalar cells bound to an immutable native image. No source file
/// is retained, no provider executes and planning performs no durable writes.
pub struct NativeEditor {
    image: NativeImage,
    snapshot: Snapshot,
}
impl NativeEditor {
    #[cfg(feature = "journal")]
    pub(super) fn journal_profile(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(UpdateProfile, JournalRoute), NativeEditError> {
        if self.image.select_native_historical_present_fixed_table(
            self.snapshot.name(),
            budget,
            cancelled,
        )? {
            return match self.image.check_native_historical_present_fixed_table(
                self.snapshot.name(),
                budget,
                cancelled,
            ) {
                Ok(()) => Ok((
                    UpdateProfile::StoredScalar,
                    JournalRoute::HistoricalPresentFixed,
                )),
                Err(EditError::Unsupported(_)) => Ok((
                    UpdateProfile::Unavailable,
                    JournalRoute::HistoricalPresentFixed,
                )),
                Err(error) => Err(error.into()),
            };
        }
        let ordinary = match self.image.select_native_ordinary_fixed_table(
            self.snapshot.name(),
            budget,
            cancelled,
        ) {
            Ok(selected) => selected,
            Err(EditError::Unsupported(_)) => {
                return Ok((UpdateProfile::Unavailable, JournalRoute::Ordinary));
            }
            Err(error) => return Err(error.into()),
        };
        if ordinary {
            let fitted = match self.image.select_native_ordinary_fitted_table(
                self.snapshot.name(),
                budget,
                cancelled,
            ) {
                Ok(selected) => selected,
                Err(EditError::Unsupported(_)) => {
                    return Ok((UpdateProfile::Unavailable, JournalRoute::Ordinary));
                }
                Err(error) => return Err(error.into()),
            };
            let checked = if fitted {
                self.image.check_native_ordinary_fitted_table(
                    self.snapshot.name(),
                    budget,
                    cancelled,
                )
            } else {
                self.image.check_native_ordinary_fixed_table(
                    self.snapshot.name(),
                    budget,
                    cancelled,
                )
            };
            return match checked {
                Ok(()) => Ok((UpdateProfile::StoredScalar, JournalRoute::Ordinary)),
                Err(EditError::Unsupported(_)) => {
                    Ok((UpdateProfile::Unavailable, JournalRoute::Ordinary))
                }
                Err(error) => Err(error.into()),
            };
        }
        self.existing_journal_profile(budget, cancelled)
            .map(|profile| (profile, JournalRoute::Existing))
    }

    #[cfg(feature = "journal")]
    fn existing_journal_profile(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<UpdateProfile, NativeEditError> {
        let fixed = match self.image.select_native_before_change_fixed_table(
            self.snapshot.name(),
            budget,
            cancelled,
        ) {
            Ok(selected) => selected,
            Err(EditError::Unsupported(_)) => {
                return Ok(UpdateProfile::Unavailable);
            }
            Err(error) => return Err(error.into()),
        };
        if fixed {
            return match self.image.check_native_before_change_fixed_table(
                self.snapshot.name(),
                budget,
                cancelled,
            ) {
                Ok(()) => Ok(UpdateProfile::StoredScalar),
                Err(EditError::Unsupported(_)) => Ok(UpdateProfile::Unavailable),
                Err(error) => Err(error.into()),
            };
        }
        let profile = self.update_profile();
        if profile != UpdateProfile::Unavailable {
            return Ok(profile);
        }
        if !matches!(
            self.image.catalog().header().format(),
            FileFormat::Ace2016 | FileFormat::Ace2019
        ) {
            return Ok(UpdateProfile::Unavailable);
        }
        match self.image.check_native_before_change_int32_table(
            self.snapshot.name(),
            budget,
            cancelled,
        ) {
            Ok(()) => Ok(UpdateProfile::ExistingInt32),
            Err(EditError::Unsupported(_)) => Ok(UpdateProfile::Unavailable),
            Err(error) => Err(error.into()),
        }
    }

    #[cfg(feature = "journal")]
    pub(super) fn new_journal(
        image: NativeImage,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeEditError> {
        if matches!(
            image.catalog().header().format(),
            FileFormat::Ace2016 | FileFormat::Ace2019
        ) {
            let projected = snapshot::capture_projected(&image, table, budget, cancelled)?;
            Ok(Self {
                image,
                snapshot: projected.snapshot,
            })
        } else {
            Self::new(image, table, budget, cancelled)
        }
    }

    /// Read one table from the exact captured image and issue snapshot-local row
    /// handles. ACE reads remain available but its write profile is Unavailable.
    pub fn new(
        image: NativeImage,
        table: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeEditError> {
        let snapshot = snapshot::capture(&image, table, budget, cancelled)?;
        Ok(Self { image, snapshot })
    }
}

impl EditableTable for NativeEditor {
    type Error = NativeEditError;
    type Plan = NativeEditPlan;
    fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }
    fn update_profile(&self) -> UpdateProfile {
        if self.image.catalog().header().format() == FileFormat::Jet4 {
            UpdateProfile::ExistingInt32
        } else {
            UpdateProfile::Unavailable
        }
    }
    fn plan_updates(
        self,
        column: &str,
        updates: &[CellUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self::Plan, Self::Error> {
        let native = self.int32_updates(column, updates, budget, cancelled)?;
        Ok(self.image.plan_int32_updates(
            self.snapshot.name(),
            column,
            &native,
            budget,
            cancelled,
        )?)
    }
}

impl NativeEditor {
    /// Share the exact snapshot-handle and carrier conversion across normal and
    /// recoverable plans. This creates requests, never persistence permission.
    pub(super) fn int32_updates(
        &self,
        column: &str,
        updates: &[CellUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<Int32Update>, NativeEditError> {
        if cancelled() {
            return Err(InspectError::Cancelled.into());
        }
        budget
            .charge(Resource::Steps, 1)
            .map_err(InspectError::from)?;
        let position = self
            .snapshot
            .columns()
            .iter()
            .position(|candidate| {
                self.snapshot
                    .policy()
                    .names
                    .equals(candidate.name(), column)
            })
            .ok_or(NativeEditError::Unsupported)?;
        budget
            .charge_allocation(updates.len() as u64, size_of::<Int32Update>() as u64)
            .map_err(InspectError::from)?;
        let mut native = Vec::new();
        native
            .try_reserve_exact(updates.len())
            .map_err(|_| InspectError::Allocation)?;
        for update in updates {
            if cancelled() {
                return Err(InspectError::Cancelled.into());
            }
            budget
                .charge(Resource::Steps, 1)
                .map_err(InspectError::from)?;
            let ordinal = self.snapshot.ordinal(update.row())?;
            let row = self.snapshot.resolve(update.row())?;
            let (Some(Value::Int32(expected)), Value::Int32(value)) =
                (row.get(position), update.value())
            else {
                return Err(NativeEditError::Unsupported);
            };
            native.push(Int32Update::new(ordinal, *expected, *value));
        }
        Ok(native)
    }

    #[cfg(feature = "journal")]
    pub(super) fn plan_recoverable(
        self,
        column: &str,
        updates: Vec<Int32Update>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
        self.image.plan_recoverable_int32_updates(
            self.snapshot.name(),
            column,
            updates,
            budget,
            cancelled,
        )
    }
}

#[cfg(feature = "journal")]
impl NativeEditor {
    pub(super) fn plan_recoverable_fixed(
        self,
        groups: Vec<NativeFixedIntentGroup>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
        self.image
            .plan_recoverable_fixed_updates(self.snapshot.name(), groups, budget, cancelled)
    }
}

#[cfg(feature = "journal")]
impl NativeEditor {
    pub(super) fn plan_recoverable_historical_present_fixed(
        self,
        groups: Vec<NativeFixedIntentGroup>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
        self.image
            .plan_recoverable_historical_present_fixed_updates(
                self.snapshot.name(),
                groups,
                budget,
                cancelled,
            )
    }

    pub(super) fn plan_recoverable_ordinary_fixed(
        self,
        groups: Vec<NativeFixedIntentGroup>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
        self.image.plan_recoverable_ordinary_fixed_updates(
            self.snapshot.name(),
            groups,
            budget,
            cancelled,
        )
    }
}

#[cfg(feature = "journal")]
impl NativeEditor {
    pub(super) fn plan_recoverable_ordinary_variable(
        self,
        groups: Vec<NativeFittedIntentGroup>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
        self.image.plan_recoverable_ordinary_variable_updates(
            self.snapshot.name(),
            groups,
            budget,
            cancelled,
        )
    }
}
