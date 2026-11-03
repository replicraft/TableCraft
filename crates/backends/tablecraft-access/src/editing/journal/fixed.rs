//! Exact original-snapshot carriers for the normal fixed batch intent.
use super::{Error, JournalRoute, NativeJournalEditor};
use crate::editing::NativeEditError;
use tablecraft_ace::{
    FixedScalarUpdate, InspectError, NativeFixedIntentGroup, NativeImage, RecoverableEditError,
    RecoverableNativeEditPlan,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::{CellUpdate, ColumnUpdates, EditableTable, UpdateProfile};
use tablecraft_schema::ScalarType;

impl NativeJournalEditor {
    /// Select before conversion from actual original carriers, never by retry.
    pub(super) fn legacy_int32(
        &self,
        column: &str,
        updates: &[CellUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, NativeEditError> {
        if self.route == JournalRoute::Ordinary {
            if self.profile != UpdateProfile::StoredScalar {
                return Err(NativeEditError::Unsupported);
            }
            // Ordinary single Int32 and empty requests retain their distinct
            // native closure/TCNI0004 route rather than the positive-event route.
            return Ok(false);
        }
        if self.profile != UpdateProfile::StoredScalar {
            return Ok(true);
        }
        let position = self.fixed_position(column, budget, cancelled)?;
        if self.snapshot().columns()[position].data_type() != ScalarType::Int32 {
            return Ok(false);
        }
        for update in updates {
            check(budget, cancelled)?;
            let row = self.snapshot().resolve(update.row())?;
            if !matches!(row.get(position), Some(Value::Int32(_)))
                || !matches!(update.value(), Value::Int32(_))
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn fixed_position(
        &self,
        name: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<usize, NativeEditError> {
        let mut selected = None;
        for (position, column) in self.snapshot().columns().iter().enumerate() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, column.name().len().max(name.len()) as u64)
                .map_err(InspectError::from)?;
            if self.snapshot().policy().names.equals(column.name(), name)
                && selected.replace(position).is_some()
            {
                return Err(NativeEditError::Unsupported);
            }
        }
        selected.ok_or(NativeEditError::Unsupported)
    }

    /// Retain distinct original matched rows for the session's prepaid report.
    pub(crate) fn plan_columns_with_count(
        self,
        columns: &[ColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(RecoverableNativeEditPlan, usize), Error> {
        let _span = tracing::debug_span!("access.journal.plan_fixed_columns").entered();
        check(budget, cancelled)?;
        if self.profile != UpdateProfile::StoredScalar {
            let [column] = columns else {
                return Err(NativeEditError::Unsupported.into());
            };
            let matched = column.updates.len();
            let plan = self.plan_updates(column.column, column.updates, budget, cancelled)?;
            return Ok((plan, matched));
        }
        if let [column] = columns
            && self.legacy_int32(column.column, column.updates, budget, cancelled)?
        {
            let matched = column.updates.len();
            let plan = self.plan_updates(column.column, column.updates, budget, cancelled)?;
            return Ok((plan, matched));
        }
        if columns.is_empty() || columns.len() > NativeImage::MAX_FIXED_BATCH_GROUPS {
            return Err(NativeEditError::Unsupported.into());
        }
        let mut count = 0_usize;
        for column in columns {
            check(budget, cancelled)?;
            count = count
                .checked_add(column.updates.len())
                .filter(|count| *count <= NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS)
                .ok_or(InspectError::StructureLimit)
                .map_err(NativeEditError::from)?;
        }
        if self.route == JournalRoute::Ordinary
            && self.variable_columns_requested(columns, budget, cancelled)?
        {
            return self.plan_variable_columns_with_count(columns, count, budget, cancelled);
        }
        let mut positions = vector(columns.len(), budget)?;
        let mut groups = vector(columns.len(), budget)?;
        let mut rows = vector(count, budget)?;
        for column in columns {
            check(budget, cancelled)?;
            let position = self.fixed_position(column.column, budget, cancelled)?;
            budget
                .charge(Resource::Steps, positions.len() as u64)
                .map_err(InspectError::from)
                .map_err(NativeEditError::from)?;
            if positions.contains(&position) {
                return Err(NativeEditError::Unsupported.into());
            }
            positions.push(position);
            let target = &self.snapshot().columns()[position];
            let kind = target.data_type();
            let ieee = self.route == JournalRoute::Ordinary
                && matches!(kind, ScalarType::Single | ScalarType::Double);
            let small_integer = self.route == JournalRoute::Ordinary
                && matches!(kind, ScalarType::Byte | ScalarType::Int16);
            let guid = self.route == JournalRoute::Ordinary && kind == ScalarType::Guid;
            if !ieee
                && !small_integer
                && !guid
                && !matches!(
                    kind,
                    ScalarType::Boolean
                        | ScalarType::Int32
                        | ScalarType::Int64
                        | ScalarType::Currency
                        | ScalarType::DateTime
                )
            {
                return Err(NativeEditError::Unsupported.into());
            }
            let mut requests: Vec<FixedScalarUpdate> = vector(column.updates.len(), budget)?;
            for update in column.updates {
                check(budget, cancelled)?;
                let ordinal = self
                    .snapshot()
                    .ordinal(update.row())
                    .map_err(NativeEditError::from)?;
                let original = self
                    .snapshot()
                    .resolve(update.row())
                    .map_err(NativeEditError::from)?
                    .get(position)
                    .ok_or(NativeEditError::Unsupported)?;
                budget
                    .charge(Resource::Steps, requests.len() as u64)
                    .map_err(InspectError::from)
                    .map_err(NativeEditError::from)?;
                if requests.iter().any(|request| request.row() == ordinal) {
                    return Err(NativeEditError::Unsupported.into());
                }
                let (expected, replacement) = if guid {
                    (
                        guid_scalar(original, kind)?,
                        guid_scalar(update.value(), kind)?,
                    )
                } else if small_integer {
                    (
                        small_integer_scalar(original, kind)?,
                        small_integer_scalar(update.value(), kind)?,
                    )
                } else if ieee {
                    (
                        ieee_scalar(original, kind)?,
                        ieee_scalar(update.value(), kind)?,
                    )
                } else {
                    (scalar(original, kind)?, scalar(update.value(), kind)?)
                };
                requests.push(FixedScalarUpdate::new(ordinal, expected, replacement));
                rows.push(ordinal);
            }
            let group = if guid {
                NativeFixedIntentGroup::new_ordinary_guid(
                    target.name(),
                    requests,
                    budget,
                    cancelled,
                )
            } else if small_integer {
                NativeFixedIntentGroup::new_ordinary_scalar(
                    target.name(),
                    requests,
                    budget,
                    cancelled,
                )
            } else if ieee {
                NativeFixedIntentGroup::new_ordinary_ieee(
                    target.name(),
                    requests,
                    budget,
                    cancelled,
                )
            } else {
                NativeFixedIntentGroup::new(target.name(), requests, budget, cancelled)
            };
            groups.push(group.map_err(RecoverableEditError::from)?);
        }
        let matched = matched_rows(rows, budget, cancelled)?;
        let plan = match self.route {
            JournalRoute::Ordinary => self
                .editor
                .plan_recoverable_ordinary_fixed(groups, budget, cancelled)?,
            JournalRoute::Existing => self
                .editor
                .plan_recoverable_fixed(groups, budget, cancelled)?,
        };
        Ok((plan, matched))
    }
}

fn guid_scalar(value: &Value, kind: ScalarType) -> Result<Value, NativeEditError> {
    Ok(match (kind, value) {
        (ScalarType::Guid, Value::Guid(value)) => Value::Guid(*value),
        (ScalarType::Guid, Value::Null) => Value::Null,
        _ => return Err(NativeEditError::Unsupported),
    })
}

fn small_integer_scalar(value: &Value, kind: ScalarType) -> Result<Value, NativeEditError> {
    Ok(match (kind, value) {
        (ScalarType::Byte, Value::Byte(value)) => Value::Byte(*value),
        (ScalarType::Int16, Value::Int16(value)) => Value::Int16(*value),
        (ScalarType::Byte | ScalarType::Int16, Value::Null) => Value::Null,
        _ => return Err(NativeEditError::Unsupported),
    })
}

fn ieee_scalar(value: &Value, kind: ScalarType) -> Result<Value, NativeEditError> {
    Ok(match (kind, value) {
        (ScalarType::Single, Value::Single(value)) => Value::Single(*value),
        (ScalarType::Double, Value::Double(value)) => Value::Double(*value),
        (ScalarType::Single | ScalarType::Double, Value::Null) => Value::Null,
        _ => return Err(NativeEditError::Unsupported),
    })
}

pub(super) fn scalar(value: &Value, kind: ScalarType) -> Result<Value, NativeEditError> {
    Ok(match (kind, value) {
        (ScalarType::Boolean, Value::Boolean(value)) => Value::Boolean(*value),
        (ScalarType::Int32, Value::Int32(value)) => Value::Int32(*value),
        (ScalarType::Int64, Value::Int64(value)) => Value::Int64(*value),
        (ScalarType::Currency, Value::Currency(value)) => Value::Currency(*value),
        (ScalarType::DateTime, Value::DateTime(value)) => Value::DateTime(*value),
        (
            ScalarType::Int32 | ScalarType::Int64 | ScalarType::Currency | ScalarType::DateTime,
            Value::Null,
        ) => Value::Null,
        _ => return Err(NativeEditError::Unsupported),
    })
}

pub(super) fn vector<T>(
    count: usize,
    budget: &mut ResourceBudget,
) -> Result<Vec<T>, NativeEditError> {
    budget
        .charge_allocation(count as u64, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(values)
}

pub(super) fn check(
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), NativeEditError> {
    if cancelled() {
        return Err(InspectError::Cancelled.into());
    }
    budget
        .charge(Resource::Steps, 1)
        .map_err(InspectError::from)?;
    Ok(())
}

pub(super) fn matched_rows(
    mut rows: Vec<usize>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, NativeEditError> {
    let sort_work = rows
        .len()
        .checked_mul(rows.len().max(1).ilog2() as usize + 1)
        .ok_or(InspectError::StructureLimit)
        .map_err(NativeEditError::from)?;
    budget
        .charge(Resource::Steps, sort_work as u64)
        .map_err(InspectError::from)
        .map_err(NativeEditError::from)?;
    rows.sort_unstable();
    rows.dedup();
    check(budget, cancelled)?;
    Ok(rows.len())
}
