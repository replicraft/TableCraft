//! Move original presence intent to the engine without interpreting defaults.
use super::{
    AccessEngineeringInserter, EngineeringCopy, EngineeringRequest, InspectError, NativeEditError,
    ResourceBudget, Value, check, preview,
};
use tablecraft_ace::{InsertFieldValue, OmittedRowInsert};
use tablecraft_db::{InsertValue, InsertableTable, OmittingInsertableTable};
use tablecraft_expressions::functions::dates::clock::Clock;

impl OmittingInsertableTable for AccessEngineeringInserter {
    fn plan_insert_with_omissions(
        self,
        values: Vec<InsertValue>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<EngineeringCopy, NativeEditError> {
        self.plan_presence(values, None, budget, cancelled)
    }
}

impl AccessEngineeringInserter {
    pub(super) fn plan_presence(
        self,
        values: Vec<InsertValue>,
        clock: Option<&dyn Clock>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<EngineeringCopy, NativeEditError> {
        check(budget, cancelled)?;
        if values.len() != self.columns.len() {
            return Err(NativeEditError::Unsupported);
        }

        let mut omitted = false;
        for value in &values {
            check(budget, cancelled)?;
            omitted |= matches!(value, InsertValue::Omitted);
        }
        if !omitted {
            budget
                .charge_allocation(values.len() as u64, size_of::<Value>() as u64)
                .map_err(InspectError::from)?;
            let mut supplied = Vec::new();
            supplied
                .try_reserve_exact(values.len())
                .map_err(|_| InspectError::Allocation)?;
            for value in values {
                check(budget, cancelled)?;
                let InsertValue::Supplied(value) = value else {
                    return Err(NativeEditError::Unsupported);
                };
                supplied.push(value);
            }
            // Complete rows keep their existing passive metadata/admission law.
            // No default expression is inspected merely because it is declared.
            return self.plan_insert(supplied, budget, cancelled);
        }

        budget
            .charge_allocation(values.len() as u64, size_of::<InsertFieldValue>() as u64)
            .map_err(InspectError::from)?;
        let mut fields = Vec::new();
        fields
            .try_reserve_exact(values.len())
            .map_err(|_| InspectError::Allocation)?;
        for value in values {
            check(budget, cancelled)?;
            fields.push(match value {
                InsertValue::Supplied(value) => InsertFieldValue::Supplied(value),
                InsertValue::Omitted => InsertFieldValue::Omitted,
            });
        }
        check(budget, cancelled)?;
        let rows = [OmittedRowInsert::new(fields)];

        let request = match (self.policy, clock) {
            (Some(policy), Some(clock)) => EngineeringRequest::IncrementingInsertWithClock {
                table: &self.name,
                rows: &rows,
                policy,
                clock,
            },
            (None, Some(clock)) => EngineeringRequest::InsertWithClock {
                table: &self.name,
                rows: &rows,
                clock,
            },
            (Some(policy), None) => EngineeringRequest::IncrementingInsertWithOmissions {
                table: &self.name,
                rows: &rows,
                policy,
            },
            (None, None) => EngineeringRequest::InsertWithOmissions {
                table: &self.name,
                rows: &rows,
            },
        };
        preview(self.image, request, budget, cancelled).map_err(|error| match error {
            super::super::EngineeringError::Engine(error) => NativeEditError::Edit(error),
        })
    }
}
