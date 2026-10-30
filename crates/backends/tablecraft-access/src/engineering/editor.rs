//! Snapshot-bound SQL assignments finish through one explicit engineering service.
mod routing;
use super::{EngineeringCopy, EngineeringError, EngineeringRequest, UpdateMode, preview};
use crate::editing::{NativeEditError, snapshot};
use tablecraft_ace::{
    FixedColumnUpdates, FixedScalarUpdate, InspectError, NativeImage, SlotScalarUpdate,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::{
    CellUpdate, ColumnUpdates, EditableBatch, EditableTable, Snapshot, UpdateProfile,
};
use tablecraft_schema::ScalarType;

/// One explicit engine maintenance policy, never a retry or save authorization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarUpdateMode {
    /// Select one service from resolved target metadata before native planning:
    /// fixed batch, one variable field, or a fitted fixed/variable statement.
    /// Shared Text/Binary metadata does not certify Short Text/Binary storage;
    /// the selected engine service still refuses Memo/OLE or other wrong kinds.
    Default,
    /// Fixed-field maintenance using the selected existing fitting/growth/action profile.
    Fixed(UpdateMode),
    /// One Short Text/Binary column, with optional qualified encoded INDEX growth.
    Variable {
        /// Select the existing variable-plus-index split service explicitly.
        index_growth: bool,
    },
    /// One ordinary Memo/OLE column under existing LVAL ownership rules.
    LongValue,
}

/// A separate scalar engineering editor over one original image and Snapshot.
/// This type never authorizes a normal ACE save. Every expected value is taken
/// from a validated handle in this same snapshot, and one complete assignment
/// batch finishes through the selected typed engine service.
pub struct AccessEngineeringEditor {
    image: NativeImage,
    snapshot: Snapshot,
    mode: ScalarUpdateMode,
}

impl AccessEngineeringEditor {
    /// Capture one complete scalar table under the shared operation allowance.
    /// Reading its schema/rows does not qualify native mutation or data macros;
    /// actual engine admission is still required, including for an empty batch.
    pub fn new(
        image: NativeImage,
        table: &str,
        mode: ScalarUpdateMode,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, NativeEditError> {
        let snapshot = snapshot::capture(&image, table, budget, cancelled)?;

        Ok(Self {
            image,
            snapshot,
            mode,
        })
    }
}

impl EditableTable for AccessEngineeringEditor {
    type Error = NativeEditError;
    type Plan = EngineeringCopy;

    fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    fn update_profile(&self) -> UpdateProfile {
        UpdateProfile::StoredScalar
    }

    fn plan_updates(
        self,
        column: &str,
        updates: &[CellUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<EngineeringCopy, NativeEditError> {
        check(budget, cancelled)?;
        if self.mode == ScalarUpdateMode::Default {
            return self.default_columns(&[ColumnUpdates { column, updates }], budget, cancelled);
        }

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
        self.check_mode(position)?;

        let table = self.snapshot.name();
        let request = match self.mode {
            ScalarUpdateMode::Default => return Err(NativeEditError::Unsupported),
            ScalarUpdateMode::Fixed(mode) => {
                let rows =
                    self.requests(position, updates, FixedScalarUpdate::new, budget, cancelled)?;
                return preview(
                    self.image,
                    EngineeringRequest::Update {
                        groups: &[FixedColumnUpdates::new(table, column, &rows)],
                        mode,
                    },
                    budget,
                    cancelled,
                )
                .map_err(engine_error);
            }
            ScalarUpdateMode::Variable { index_growth } => {
                let rows =
                    self.requests(position, updates, SlotScalarUpdate::new, budget, cancelled)?;
                return preview(
                    self.image,
                    EngineeringRequest::VariableUpdate {
                        table,
                        column,
                        rows: &rows,
                        index_growth,
                    },
                    budget,
                    cancelled,
                )
                .map_err(engine_error);
            }
            ScalarUpdateMode::LongValue => {
                self.requests(position, updates, SlotScalarUpdate::new, budget, cancelled)?
            }
        };

        preview(
            self.image,
            EngineeringRequest::LongValueUpdate {
                table,
                column,
                rows: &request,
            },
            budget,
            cancelled,
        )
        .map_err(engine_error)
    }
}

impl EditableBatch for AccessEngineeringEditor {
    fn plan_columns(
        self,
        columns: &[ColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<EngineeringCopy, NativeEditError> {
        check(budget, cancelled)?;
        if columns.is_empty() || columns.len() > self.snapshot.columns().len() {
            return Err(NativeEditError::Unsupported);
        }

        if self.mode == ScalarUpdateMode::Default {
            return self.default_columns(columns, budget, cancelled);
        }

        let ScalarUpdateMode::Fixed(mode) = self.mode else {
            let [column] = columns else {
                return Err(NativeEditError::Unsupported);
            };
            return self.plan_updates(column.column, column.updates, budget, cancelled);
        };

        let positions = self.validate_columns(columns, budget, cancelled)?;
        self.fixed_columns(columns, positions, mode, budget, cancelled)
    }
}

impl AccessEngineeringEditor {
    fn fixed_columns(
        self,
        columns: &[ColumnUpdates<'_>],
        positions: Vec<usize>,
        mode: UpdateMode,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<EngineeringCopy, NativeEditError> {
        budget
            .charge_allocation(
                columns.len() as u64,
                size_of::<Vec<FixedScalarUpdate>>() as u64,
            )
            .map_err(InspectError::from)?;
        let mut requests = Vec::new();
        requests
            .try_reserve_exact(columns.len())
            .map_err(|_| InspectError::Allocation)?;
        for (column, position) in columns.iter().zip(positions) {
            requests.push(self.requests(
                position,
                column.updates,
                FixedScalarUpdate::new,
                budget,
                cancelled,
            )?);
        }

        budget
            .charge_allocation(
                columns.len() as u64,
                size_of::<FixedColumnUpdates<'_>>() as u64,
            )
            .map_err(InspectError::from)?;
        let mut groups = Vec::new();
        groups
            .try_reserve_exact(columns.len())
            .map_err(|_| InspectError::Allocation)?;
        for (column, rows) in columns.iter().zip(&requests) {
            check(budget, cancelled)?;
            groups.push(FixedColumnUpdates::new(
                self.snapshot.name(),
                column.column,
                rows,
            ));
        }
        check(budget, cancelled)?;

        preview(
            self.image,
            EngineeringRequest::Update {
                groups: &groups,
                mode,
            },
            budget,
            cancelled,
        )
        .map_err(engine_error)
    }

    fn check_mode(&self, position: usize) -> Result<(), NativeEditError> {
        let target = self
            .snapshot
            .columns()
            .get(position)
            .ok_or(NativeEditError::Unsupported)?;

        match self.mode {
            ScalarUpdateMode::Default => {}
            ScalarUpdateMode::Variable { .. } | ScalarUpdateMode::LongValue => {
                if !matches!(target.data_type(), ScalarType::Text | ScalarType::Binary) {
                    return Err(NativeEditError::Unsupported);
                }
            }
            ScalarUpdateMode::Fixed(_)
                if matches!(target.data_type(), ScalarType::Text | ScalarType::Binary) =>
            {
                return Err(NativeEditError::Unsupported);
            }
            ScalarUpdateMode::Fixed(UpdateMode::IndexGrowth)
                if target.data_type() != ScalarType::Int32 =>
            {
                return Err(NativeEditError::Unsupported);
            }
            _ => {}
        }

        Ok(())
    }

    fn validate_columns(
        &self,
        columns: &[ColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<usize>, NativeEditError> {
        budget
            .charge_allocation(columns.len() as u64, size_of::<usize>() as u64)
            .map_err(InspectError::from)?;
        let mut positions = Vec::new();
        positions
            .try_reserve_exact(columns.len())
            .map_err(|_| InspectError::Allocation)?;

        let mut assignments = 0_usize;
        for column in columns {
            check(budget, cancelled)?;
            assignments = assignments
                .checked_add(column.updates.len())
                .filter(|count| *count <= NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS)
                .ok_or(InspectError::StructureLimit)?;

            for candidate in self.snapshot.columns() {
                check(budget, cancelled)?;
                budget
                    .charge(
                        Resource::Steps,
                        candidate.name().len().min(column.column.len()) as u64,
                    )
                    .map_err(InspectError::from)?;
            }
            let position = self
                .snapshot
                .columns()
                .iter()
                .position(|candidate| {
                    self.snapshot
                        .policy()
                        .names
                        .equals(candidate.name(), column.column)
                })
                .ok_or(NativeEditError::Unsupported)?;
            budget
                .charge(Resource::Steps, positions.len() as u64)
                .map_err(InspectError::from)?;
            if positions.contains(&position) {
                return Err(NativeEditError::Unsupported);
            }

            self.check_mode(position)?;
            positions.push(position);

            let target = &self.snapshot.columns()[position];
            budget
                .charge_allocation(column.updates.len() as u64, size_of::<usize>() as u64)
                .map_err(InspectError::from)?;
            let mut ordinals = Vec::new();
            ordinals
                .try_reserve_exact(column.updates.len())
                .map_err(|_| InspectError::Allocation)?;
            for update in column.updates {
                check(budget, cancelled)?;
                let ordinal = self.snapshot.ordinal(update.row())?;
                self.snapshot.resolve(update.row())?;
                budget
                    .charge(Resource::Steps, ordinals.len() as u64)
                    .map_err(InspectError::from)?;
                if ordinals.contains(&ordinal) {
                    return Err(NativeEditError::Unsupported);
                }

                ordinals.push(ordinal);
                if matches!(update.value(), Value::Null) {
                    if !target.nullable() {
                        return Err(NativeEditError::Unsupported);
                    }
                } else {
                    let work = match update.value() {
                        Value::Text(value) => value.len(),
                        Value::Binary(value) => value.len(),
                        _ => 0,
                    };
                    budget
                        .charge(Resource::Steps, work as u64)
                        .map_err(InspectError::from)?;
                    if ScalarType::of(update.value()) != Some(target.data_type())
                        || target
                            .type_descriptor()
                            .validate_value(update.value())
                            .is_err()
                    {
                        return Err(NativeEditError::Unsupported);
                    }
                }
            }
        }

        Ok(positions)
    }

    fn requests<T>(
        &self,
        position: usize,
        updates: &[CellUpdate],
        make: fn(usize, Value, Value) -> T,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<T>, NativeEditError> {
        if updates.len() > NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS {
            return Err(InspectError::StructureLimit.into());
        }

        budget
            .charge_allocation(updates.len() as u64, size_of::<T>() as u64)
            .map_err(InspectError::from)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(updates.len())
            .map_err(|_| InspectError::Allocation)?;

        for update in updates {
            check(budget, cancelled)?;
            let ordinal = self.snapshot.ordinal(update.row())?;
            let original = self
                .snapshot
                .resolve(update.row())?
                .get(position)
                .ok_or(NativeEditError::Unsupported)?;
            let expected = copy(original, budget, cancelled)?;
            let value = copy(update.value(), budget, cancelled)?;
            rows.push(make(ordinal, expected, value));
        }

        Ok(rows)
    }
}

fn copy(
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, NativeEditError> {
    check(budget, cancelled)?;
    Ok(match value {
        Value::Text(text) => {
            budget
                .charge_allocation(text.len() as u64, 1)
                .map_err(InspectError::from)?;
            budget
                .charge(Resource::Steps, text.len() as u64)
                .map_err(InspectError::from)?;
            let mut owned = String::new();
            owned
                .try_reserve_exact(text.len())
                .map_err(|_| InspectError::Allocation)?;
            owned.push_str(text);
            Value::Text(owned)
        }
        Value::Binary(bytes) => {
            budget
                .charge_allocation(bytes.len() as u64, 1)
                .map_err(InspectError::from)?;
            budget
                .charge(Resource::Steps, bytes.len() as u64)
                .map_err(InspectError::from)?;
            let mut owned = Vec::new();
            owned
                .try_reserve_exact(bytes.len())
                .map_err(|_| InspectError::Allocation)?;
            owned.extend_from_slice(bytes);
            Value::Binary(owned)
        }
        // All other stored Value variants are fixed-size, heap-free carriers.
        value => value.clone(),
    })
}

fn engine_error(error: EngineeringError) -> NativeEditError {
    match error {
        EngineeringError::Engine(error) => NativeEditError::Edit(error),
    }
}

fn check(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), NativeEditError> {
    if cancelled() {
        return Err(InspectError::Cancelled.into());
    }
    budget
        .charge(Resource::Steps, 1)
        .map_err(InspectError::from)?;
    Ok(())
}

#[cfg(test)]
pub(in crate::engineering) mod tests;
