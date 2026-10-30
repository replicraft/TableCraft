//! Choose one existing native service from the complete resolved target set.
use super::{
    AccessEngineeringEditor, EngineeringCopy, EngineeringRequest, FixedScalarUpdate, InspectError,
    NativeEditError, ResourceBudget, ScalarType, ScalarUpdateMode, SlotScalarUpdate, UpdateMode,
    check, engine_error, preview,
};
use tablecraft_ace::FittedColumnUpdates;
use tablecraft_db::ColumnUpdates;

enum Placement {
    Fitted,
    Growing,
    GrowingIndexes,
}

enum Requests {
    Fixed(Vec<FixedScalarUpdate>),
    Variable(Vec<SlotScalarUpdate>),
}

impl AccessEngineeringEditor {
    pub(super) fn default_columns(
        self,
        columns: &[ColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<EngineeringCopy, NativeEditError> {
        check(budget, cancelled)?;
        if columns.is_empty() || columns.len() > self.snapshot.columns().len() {
            return Err(NativeEditError::Unsupported);
        }

        let positions = self.validate_columns(columns, budget, cancelled)?;
        let variable_count = self.variable_count(&positions, budget, cancelled)?;
        if variable_count == 0 {
            return self.fixed_columns(columns, positions, UpdateMode::Fitted, budget, cancelled);
        }

        // Text/Binary is routing metadata, not native Short Text/Binary authority.
        // Each selected engine entry point still checks its real storage kind.
        if let [column] = columns {
            if self.ordinary_long_value_column(positions[0], budget, cancelled)? {
                let rows = self.requests(
                    positions[0],
                    column.updates,
                    SlotScalarUpdate::new,
                    budget,
                    cancelled,
                )?;
                return preview(
                    self.image,
                    EngineeringRequest::LongValueUpdate {
                        table: self.snapshot.name(),
                        column: column.column,
                        rows: &rows,
                    },
                    budget,
                    cancelled,
                )
                .map_err(engine_error);
            }
            let rows = self.requests(
                positions[0],
                column.updates,
                SlotScalarUpdate::new,
                budget,
                cancelled,
            )?;
            return preview(
                self.image,
                EngineeringRequest::VariableUpdate {
                    table: self.snapshot.name(),
                    column: column.column,
                    rows: &rows,
                    index_growth: false,
                },
                budget,
                cancelled,
            )
            .map_err(engine_error);
        }

        self.joint_columns(columns, positions, Placement::Fitted, budget, cancelled)
    }

    pub(super) fn variable_columns(
        self,
        columns: &[ColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<EngineeringCopy, NativeEditError> {
        check(budget, cancelled)?;
        let ScalarUpdateMode::Variable { index_growth } = self.mode else {
            return Err(NativeEditError::Unsupported);
        };
        if columns.len() < 2 || columns.len() > self.snapshot.columns().len() {
            return Err(NativeEditError::Unsupported);
        }
        let positions = self.validate_columns(columns, budget, cancelled)?;
        if self.variable_count(&positions, budget, cancelled)? == 0 {
            return Err(NativeEditError::Unsupported);
        }
        let placement = if index_growth {
            Placement::GrowingIndexes
        } else {
            Placement::Growing
        };
        self.joint_columns(columns, positions, placement, budget, cancelled)
    }

    fn variable_count(
        &self,
        positions: &[usize],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<usize, NativeEditError> {
        let mut count = 0;
        for position in positions {
            check(budget, cancelled)?;
            if matches!(
                self.snapshot.columns()[*position].data_type(),
                ScalarType::Text | ScalarType::Binary
            ) {
                count += 1;
            }
        }
        Ok(count)
    }

    fn joint_columns(
        self,
        columns: &[ColumnUpdates<'_>],
        positions: Vec<usize>,
        placement: Placement,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<EngineeringCopy, NativeEditError> {
        budget
            .charge_allocation(columns.len() as u64, size_of::<Requests>() as u64)
            .map_err(InspectError::from)?;
        let mut requests = Vec::new();
        requests
            .try_reserve_exact(columns.len())
            .map_err(|_| InspectError::Allocation)?;
        for (column, position) in columns.iter().zip(positions) {
            check(budget, cancelled)?;
            let rows = if matches!(
                self.snapshot.columns()[position].data_type(),
                ScalarType::Text | ScalarType::Binary
            ) {
                Requests::Variable(self.requests(
                    position,
                    column.updates,
                    SlotScalarUpdate::new,
                    budget,
                    cancelled,
                )?)
            } else {
                Requests::Fixed(self.requests(
                    position,
                    column.updates,
                    FixedScalarUpdate::new,
                    budget,
                    cancelled,
                )?)
            };
            requests.push(rows);
        }

        budget
            .charge_allocation(
                columns.len() as u64,
                size_of::<FittedColumnUpdates<'_>>() as u64,
            )
            .map_err(InspectError::from)?;
        let mut groups = Vec::new();
        groups
            .try_reserve_exact(columns.len())
            .map_err(|_| InspectError::Allocation)?;
        for (column, rows) in columns.iter().zip(&requests) {
            check(budget, cancelled)?;
            groups.push(match rows {
                Requests::Fixed(updates) => FittedColumnUpdates::Fixed {
                    column: column.column,
                    updates,
                },
                Requests::Variable(updates) => FittedColumnUpdates::Variable {
                    column: column.column,
                    updates,
                },
            });
        }
        check(budget, cancelled)?;

        let request = match placement {
            Placement::Fitted => EngineeringRequest::FittedUpdate {
                table: self.snapshot.name(),
                groups: &groups,
            },
            Placement::Growing => EngineeringRequest::MixedUpdate {
                table: self.snapshot.name(),
                groups: &groups,
            },
            Placement::GrowingIndexes => EngineeringRequest::MixedSplitUpdate {
                table: self.snapshot.name(),
                groups: &groups,
            },
        };
        preview(self.image, request, budget, cancelled).map_err(engine_error)
    }
}
