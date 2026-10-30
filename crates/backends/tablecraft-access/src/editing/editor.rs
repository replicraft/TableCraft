use super::{NativeEditError, snapshot};
use tablecraft_ace::{FileFormat, InspectError, Int32Update, NativeEditPlan, NativeImage};
#[cfg(feature = "journal")]
use tablecraft_ace::{RecoverableEditError, RecoverableNativeEditPlan};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::{CellUpdate, EditableTable, Snapshot, UpdateProfile};

/// One complete scalar table bound to an immutable native image. No source file
/// is retained, no provider executes and planning performs no durable writes.
pub struct NativeEditor {
    image: NativeImage,
    snapshot: Snapshot,
}
impl NativeEditor {
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
