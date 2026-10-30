//! Original named presence maps once to existing complete native batch services.
use super::{AccessEngineeringInserter, NativeEditError, ScalarInsertMode, check};
use crate::engineering::{EngineeringCopy, EngineeringRequest, NamedInsertMode, preview};
use tablecraft_ace::{
    AutoNumberRowInsert, InsertFieldValue, InspectError, NativeImage, OmittedRowInsert, RowInsert,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::InsertableTable;
use tablecraft_expressions::functions::dates::clock::Clock;

const MAX_ROWS: usize = 4096;

pub(in crate::engineering) struct Request<'a> {
    pub fields: &'a [String],
    pub rows: Vec<Vec<InsertFieldValue>>,
    pub mode: NamedInsertMode,
    pub clock: Option<&'a dyn Clock>,
}

impl AccessEngineeringInserter {
    pub(in crate::engineering) fn preview_named(
        image: NativeImage,
        table: &str,
        request: Request<'_>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<EngineeringCopy, NativeEditError> {
        check(budget, cancelled)?;
        let Request {
            fields,
            rows,
            mode,
            clock,
        } = request;

        if rows.is_empty() || rows.len() > MAX_ROWS {
            return Err(NativeEditError::Unsupported);
        }

        let scalar_mode = match mode {
            NamedInsertMode::Ordinary => ScalarInsertMode::Explicit,
            NamedInsertMode::Incrementing | NamedInsertMode::IncrementingLast(_) => {
                ScalarInsertMode::Incrementing
            }
        };

        let mut target = Self::new(image, table, scalar_mode, budget, cancelled)?;
        if let NamedInsertMode::IncrementingLast(policy) = mode {
            // Declaration/position came from the same original native binding;
            // the engine independently rechecks the supplied LAST precondition.
            target.policy = Some(policy);
        }

        let positions = target.bind_fields(fields, budget, cancelled)?;
        target.check_rows(&positions, &rows, budget, cancelled)?;

        let (mapped, omitted) =
            map_rows(target.columns.len(), &positions, rows, budget, cancelled)?;

        if !omitted {
            return target.complete(mapped, budget, cancelled);
        }

        let mut requests = Vec::new();
        reserve(&mut requests, mapped.len(), budget)?;
        for row in mapped {
            check(budget, cancelled)?;
            requests.push(OmittedRowInsert::new(row));
        }

        let request = match (target.policy, clock) {
            (Some(policy), Some(clock)) => EngineeringRequest::IncrementingInsertWithClock {
                table: &target.name,
                rows: &requests,
                policy,
                clock,
            },
            (Some(policy), None) => EngineeringRequest::IncrementingInsertWithOmissions {
                table: &target.name,
                rows: &requests,
                policy,
            },
            (None, Some(clock)) => EngineeringRequest::InsertWithClock {
                table: &target.name,
                rows: &requests,
                clock,
            },
            (None, None) => EngineeringRequest::InsertWithOmissions {
                table: &target.name,
                rows: &requests,
            },
        };

        check(budget, cancelled)?;
        preview(target.image, request, budget, cancelled).map_err(Into::into)
    }

    fn bind_fields(
        &self,
        fields: &[String],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<usize>, NativeEditError> {
        let policy = self.schema_policy();
        if fields.is_empty() || fields.len() > self.columns.len() {
            return Err(NativeEditError::Unsupported);
        }

        let mut positions = Vec::new();
        reserve(&mut positions, fields.len(), budget)?;
        for field in fields {
            check(budget, cancelled)?;
            if field.is_empty() || field.len() > policy.max_name_bytes {
                return Err(NativeEditError::Unsupported);
            }

            let mut found = None;
            for (position, column) in self.columns.iter().enumerate() {
                check(budget, cancelled)?;
                budget
                    .charge(Resource::Steps, field.len().max(column.name().len()) as u64)
                    .map_err(InspectError::from)?;
                if policy.names.equals(field, column.name()) && found.replace(position).is_some() {
                    return Err(NativeEditError::Unsupported);
                }
            }

            // Generated primary fields were removed by the actual native binding.
            let position = found.ok_or(NativeEditError::Unsupported)?;
            budget
                .charge(Resource::Steps, positions.len() as u64)
                .map_err(InspectError::from)?;
            if positions.contains(&position) {
                return Err(NativeEditError::Unsupported);
            }
            positions.push(position);
        }

        Ok(positions)
    }

    fn check_rows(
        &self,
        positions: &[usize],
        rows: &[Vec<InsertFieldValue>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), NativeEditError> {
        for row in rows {
            check(budget, cancelled)?;
            if row.len() != positions.len() {
                return Err(NativeEditError::Unsupported);
            }

            for (position, value) in positions.iter().copied().zip(row) {
                check(budget, cancelled)?;
                if let InsertFieldValue::Supplied(value) = value {
                    let bytes = match value {
                        Value::Text(text) => text.len(),
                        Value::Binary(bytes) => bytes.len(),
                        _ => 1,
                    };
                    budget
                        .charge(Resource::Steps, bytes as u64)
                        .map_err(InspectError::from)?;
                    if self.columns[position]
                        .type_descriptor()
                        .validate_value(value)
                        .is_err()
                    {
                        return Err(NativeEditError::Unsupported);
                    }
                }
            }
        }

        Ok(())
    }

    fn complete(
        self,
        rows: Vec<Vec<InsertFieldValue>>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<EngineeringCopy, NativeEditError> {
        fn supplied(
            row: Vec<InsertFieldValue>,
            budget: &mut ResourceBudget,
            cancelled: &dyn Fn() -> bool,
        ) -> Result<Vec<Value>, NativeEditError> {
            let mut values = Vec::new();
            reserve(&mut values, row.len(), budget)?;
            for value in row {
                check(budget, cancelled)?;
                let InsertFieldValue::Supplied(value) = value else {
                    return Err(NativeEditError::Unsupported);
                };
                values.push(value);
            }

            Ok(values)
        }

        let request = if let Some(policy) = self.policy {
            let mut requests = Vec::new();
            reserve(&mut requests, rows.len(), budget)?;
            for row in rows {
                requests.push(AutoNumberRowInsert::new(supplied(row, budget, cancelled)?));
            }

            preview(
                self.image,
                EngineeringRequest::IncrementingInsert {
                    table: &self.name,
                    rows: &requests,
                    policy,
                },
                budget,
                cancelled,
            )
        } else {
            let mut requests = Vec::new();
            reserve(&mut requests, rows.len(), budget)?;
            for row in rows {
                requests.push(RowInsert::new(supplied(row, budget, cancelled)?));
            }

            preview(
                self.image,
                EngineeringRequest::Insert {
                    table: &self.name,
                    rows: &requests,
                },
                budget,
                cancelled,
            )
        };

        request.map_err(Into::into)
    }
}

fn reserve<T>(
    values: &mut Vec<T>,
    count: usize,
    budget: &mut ResourceBudget,
) -> Result<(), NativeEditError> {
    budget
        .charge_allocation(count as u64, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    values
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(())
}

fn map_rows(
    width: usize,
    positions: &[usize],
    rows: Vec<Vec<InsertFieldValue>>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<Vec<InsertFieldValue>>, bool), NativeEditError> {
    let mut mapped = Vec::new();
    reserve(&mut mapped, rows.len(), budget)?;
    let mut omitted = false;

    for row in rows {
        check(budget, cancelled)?;
        let mut values = Vec::new();
        reserve(&mut values, width, budget)?;
        values.resize_with(width, || InsertFieldValue::Omitted);

        for (position, value) in positions.iter().copied().zip(row) {
            check(budget, cancelled)?;
            values[position] = value;
        }

        for value in &values {
            check(budget, cancelled)?;
            omitted |= matches!(value, InsertFieldValue::Omitted);
        }
        mapped.push(values);
    }

    Ok((mapped, omitted))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_row_mapping_moves_text_binary_and_null_without_payload_clones() {
        let text = "original payload".repeat(1024);
        let bytes = vec![0xa7; 4096];
        let text_pointer = text.as_ptr();
        let binary_pointer = bytes.as_ptr();
        let rows = vec![vec![
            InsertFieldValue::Supplied(Value::Text(text)),
            InsertFieldValue::Supplied(Value::Binary(bytes)),
            InsertFieldValue::Supplied(Value::Null),
        ]];

        let mut budget = ResourceBudget::new(1_000_000, 100, 1_000_000);
        let (mapped, omitted) = map_rows(4, &[2, 0, 1], rows, &mut budget, &|| false).unwrap();
        assert!(omitted);

        let InsertFieldValue::Supplied(Value::Text(text)) = &mapped[0][2] else {
            panic!("mapped Text");
        };
        assert_eq!(text.as_ptr(), text_pointer);

        let InsertFieldValue::Supplied(Value::Binary(bytes)) = &mapped[0][0] else {
            panic!("mapped Binary");
        };
        assert_eq!(bytes.as_ptr(), binary_pointer);
        assert!(matches!(
            mapped[0][1],
            InsertFieldValue::Supplied(Value::Null)
        ));
        assert!(matches!(mapped[0][3], InsertFieldValue::Omitted));
    }
}
