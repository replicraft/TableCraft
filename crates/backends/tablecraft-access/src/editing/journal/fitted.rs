//! Original variable requests retain exact carriers before native placement selection.
use super::{
    Error, JournalRoute, NativeJournalEditor,
    fixed::{check, matched_rows, scalar, vector},
};
use crate::editing::NativeEditError;
use tablecraft_ace::{
    FixedScalarUpdate, InspectError, NativeFittedEditIntent, NativeFittedIntentGroup,
    NativeFittedUpdates, RecoverableEditError, RecoverableNativeEditPlan, SlotScalarUpdate,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::{ColumnUpdates, EditableTable, UpdateProfile};
use tablecraft_schema::ScalarType;

const COPY_BYTES: usize = 4096;

#[cfg(feature = "ace16-create")]
fn retained_fixed_scalar(kind: ScalarType) -> bool {
    matches!(
        kind,
        ScalarType::Byte
            | ScalarType::Int16
            | ScalarType::Int64
            | ScalarType::Single
            | ScalarType::Double
            | ScalarType::DateTime
    )
}

#[cfg(feature = "ace16-create")]
fn retained_scalar(value: &Value, kind: ScalarType) -> Result<Value, NativeEditError> {
    Ok(match (kind, value) {
        (ScalarType::Byte, Value::Byte(value)) => Value::Byte(*value),
        (ScalarType::Int16, Value::Int16(value)) => Value::Int16(*value),
        (ScalarType::Int64, Value::Int64(value)) => Value::Int64(*value),
        (ScalarType::Single, Value::Single(value)) if value.to_float().is_finite() => {
            Value::Single(*value)
        }
        (ScalarType::Double, Value::Double(value)) if value.to_float().is_finite() => {
            Value::Double(*value)
        }
        (ScalarType::DateTime, Value::DateTime(value)) => Value::DateTime(*value),
        (kind, Value::Null) if retained_fixed_scalar(kind) => Value::Null,
        _ => return Err(NativeEditError::Unsupported),
    })
}

fn scalar_request(
    value: &Value,
    kind: ScalarType,
    route: JournalRoute,
) -> Result<Value, NativeEditError> {
    #[cfg(feature = "ace16-create")]
    if route == JournalRoute::RetainedRows && retained_fixed_scalar(kind) {
        return retained_scalar(value, kind);
    }
    #[cfg(not(feature = "ace16-create"))]
    let _ = route;
    scalar(value, kind)
}

impl NativeJournalEditor {
    /// Original target metadata chooses the request category before conversion.
    /// Empty Text/Binary groups retain their fitted semantics and are not dropped.
    pub(super) fn variable_columns_requested(
        &self,
        columns: &[ColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, NativeEditError> {
        let mut variable = false;
        for column in columns {
            check(budget, cancelled)?;
            let position = self.fixed_position(column.column, budget, cancelled)?;
            variable |= matches!(
                self.snapshot().columns()[position].data_type(),
                ScalarType::Text | ScalarType::Binary
            );
        }
        Ok(variable)
    }

    pub(super) fn plan_variable_columns_with_count(
        self,
        columns: &[ColumnUpdates<'_>],
        count: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(RecoverableNativeEditPlan, usize), Error> {
        let _span = tracing::debug_span!("access.journal.plan_variable_columns").entered();
        check(budget, cancelled)?;
        if (self.route != JournalRoute::Ordinary
            && !self.route.historical_rows()
            && !self.route.row_updates())
            || self.profile != UpdateProfile::StoredScalar
        {
            return Err(NativeEditError::Unsupported.into());
        }
        let mut positions = vector(columns.len(), budget)?;
        let mut groups = vector(columns.len(), budget)?;
        let mut rows = vector(count, budget)?;
        // Resolve every original target before copying any variable payload.
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
            let kind = self.snapshot().columns()[position].data_type();
            if self.route.historical_rows() && (columns.len() != 1 || kind != ScalarType::Text) {
                return Err(NativeEditError::Unsupported.into());
            }
            #[cfg(feature = "ace16-create")]
            if self.route == JournalRoute::RetainedOpaqueRows
                && (columns.len() != 1 || kind != ScalarType::Int32)
            {
                return Err(NativeEditError::Unsupported.into());
            }
            if self.route.row_updates() {
                let supported = matches!(kind, ScalarType::Int32 | ScalarType::Text);
                #[cfg(feature = "ace16-create")]
                let supported = supported
                    || self.route == JournalRoute::RetainedRows
                        && (matches!(kind, ScalarType::Currency | ScalarType::Boolean)
                            || retained_fixed_scalar(kind));
                if columns.len() != 1 || !supported {
                    return Err(NativeEditError::Unsupported.into());
                }
            }
            #[cfg(feature = "ace16-create")]
            let new_scalar =
                self.route == JournalRoute::RetainedRows && retained_fixed_scalar(kind);
            #[cfg(not(feature = "ace16-create"))]
            let new_scalar = false;
            if !new_scalar
                && !matches!(
                    kind,
                    ScalarType::Boolean
                        | ScalarType::Int32
                        | ScalarType::Int64
                        | ScalarType::Currency
                        | ScalarType::DateTime
                        | ScalarType::Text
                        | ScalarType::Binary
                )
            {
                return Err(NativeEditError::Unsupported.into());
            }
            positions.push(position);
        }
        let maximum_variable_bytes = if self.route.retained_rows() {
            NativeFittedEditIntent::MAX_RETAINED_VARIABLE_BYTES
        } else {
            NativeFittedEditIntent::MAX_VARIABLE_BYTES
        };
        for (column, &position) in columns.iter().zip(&positions) {
            check(budget, cancelled)?;
            let target = &self.snapshot().columns()[position];
            let kind = target.data_type();
            let mut requests = if matches!(kind, ScalarType::Text | ScalarType::Binary) {
                NativeFittedUpdates::Variable(vector(column.updates.len(), budget)?)
            } else {
                NativeFittedUpdates::Fixed(vector(column.updates.len(), budget)?)
            };
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
                let length = match &requests {
                    NativeFittedUpdates::Fixed(updates) => updates.len(),
                    NativeFittedUpdates::Variable(updates) => updates.len(),
                };
                budget
                    .charge(Resource::Steps, length as u64)
                    .map_err(InspectError::from)
                    .map_err(NativeEditError::from)?;
                let duplicate = match &requests {
                    NativeFittedUpdates::Fixed(updates) => {
                        updates.iter().any(|cell| cell.row() == ordinal)
                    }
                    NativeFittedUpdates::Variable(updates) => {
                        updates.iter().any(|cell| cell.row() == ordinal)
                    }
                };
                if duplicate {
                    return Err(NativeEditError::Unsupported.into());
                }
                match &mut requests {
                    NativeFittedUpdates::Fixed(updates) => {
                        updates.push(FixedScalarUpdate::new(
                            ordinal,
                            scalar_request(original, kind, self.route)?,
                            scalar_request(update.value(), kind, self.route)?,
                        ));
                    }
                    NativeFittedUpdates::Variable(updates) => {
                        updates.push(SlotScalarUpdate::new(
                            ordinal,
                            variable(original, kind, maximum_variable_bytes, budget, cancelled)?,
                            variable(
                                update.value(),
                                kind,
                                maximum_variable_bytes,
                                budget,
                                cancelled,
                            )?,
                        ));
                    }
                }
                rows.push(ordinal);
            }
            #[cfg(feature = "ace16-create")]
            let new_scalar =
                self.route == JournalRoute::RetainedRows && retained_fixed_scalar(kind);
            #[cfg(feature = "ace16-create")]
            let group = if new_scalar {
                NativeFittedIntentGroup::new_retained_fixed_scalars(
                    target.name(),
                    requests,
                    budget,
                    cancelled,
                )
            } else if self.route.retained_rows() {
                NativeFittedIntentGroup::new_retained(target.name(), requests, budget, cancelled)
            } else {
                NativeFittedIntentGroup::new(target.name(), requests, budget, cancelled)
            };
            #[cfg(not(feature = "ace16-create"))]
            let group = if self.route.retained_rows() {
                NativeFittedIntentGroup::new_retained(target.name(), requests, budget, cancelled)
            } else {
                NativeFittedIntentGroup::new(target.name(), requests, budget, cancelled)
            };
            groups.push(group.map_err(RecoverableEditError::from)?);
        }
        let matched = matched_rows(rows, budget, cancelled)?;
        let plan = match self.route {
            JournalRoute::Ordinary => self
                .editor
                .plan_recoverable_ordinary_variable(groups, budget, cancelled)?,
            #[cfg(feature = "ace16-create")]
            JournalRoute::HistoricalRows => self
                .editor
                .plan_recoverable_historical_short_text(groups, budget, cancelled)?,
            #[cfg(feature = "ace16-create")]
            JournalRoute::CurrentRows => self
                .editor
                .plan_recoverable_current_rows(groups, budget, cancelled)?,
            #[cfg(feature = "ace16-create")]
            JournalRoute::RetainedOpaqueRows => self
                .editor
                .plan_recoverable_retained_opaque_rows(groups, budget, cancelled)?,
            #[cfg(feature = "ace16-create")]
            JournalRoute::RetainedRows => {
                if positions.first().is_some_and(|&position| {
                    retained_fixed_scalar(self.snapshot().columns()[position].data_type())
                }) {
                    self.editor
                        .plan_recoverable_retained_fixed_scalar_rows(groups, budget, cancelled)?
                } else if positions.first().is_some_and(|&position| {
                    self.snapshot().columns()[position].data_type() == ScalarType::Boolean
                }) {
                    self.editor
                        .plan_recoverable_retained_boolean_rows(groups, budget, cancelled)?
                } else if positions.first().is_some_and(|&position| {
                    self.snapshot().columns()[position].data_type() == ScalarType::Currency
                }) {
                    self.editor
                        .plan_recoverable_retained_currency_rows(groups, budget, cancelled)?
                } else {
                    self.editor
                        .plan_recoverable_retained_rows(groups, budget, cancelled)?
                }
            }
            JournalRoute::Existing => return Err(NativeEditError::Unsupported.into()),
        };
        Ok((plan, matched))
    }
}

fn variable(
    value: &Value,
    kind: ScalarType,
    maximum_bytes: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, NativeEditError> {
    check(budget, cancelled)?;
    let length = match (kind, value) {
        (ScalarType::Text | ScalarType::Binary, Value::Null) => return Ok(Value::Null),
        (ScalarType::Text, Value::Text(value)) => value.len(),
        (ScalarType::Binary, Value::Binary(value)) => value.len(),
        _ => return Err(NativeEditError::Unsupported),
    };
    // This is an intent-data ceiling, not native field/nullability admission.
    if length > maximum_bytes {
        return Err(InspectError::StructureLimit.into());
    }
    budget
        .charge_allocation(length as u64, 1)
        .map_err(InspectError::from)?;
    match value {
        Value::Text(value) => {
            let mut result = String::new();
            result
                .try_reserve_exact(length)
                .map_err(|_| InspectError::Allocation)?;
            let mut at = 0;
            while at < value.len() {
                check(budget, cancelled)?;
                let mut end = value.len().min(at + COPY_BYTES);
                while !value.is_char_boundary(end) {
                    end -= 1;
                }
                budget
                    .charge(Resource::Steps, (end - at) as u64)
                    .map_err(InspectError::from)?;
                result.push_str(&value[at..end]);
                at = end;
            }
            check(budget, cancelled)?;
            Ok(Value::Text(result))
        }
        Value::Binary(value) => {
            let mut result = Vec::new();
            result
                .try_reserve_exact(length)
                .map_err(|_| InspectError::Allocation)?;
            for chunk in value.chunks(COPY_BYTES) {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, chunk.len() as u64)
                    .map_err(InspectError::from)?;
                result.extend_from_slice(chunk);
            }
            check(budget, cancelled)?;
            Ok(Value::Binary(result))
        }
        _ => Err(NativeEditError::Unsupported),
    }
}
