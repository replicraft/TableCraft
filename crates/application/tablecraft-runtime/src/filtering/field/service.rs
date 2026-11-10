//! A common-filter description grants no permission; Apply binds everything again.
use super::{FieldFilterDescription, FieldFilterOperator, FieldFilterTarget, operators};
use crate::filtering::{
    Criterion, FILTER_STEPS, FilterError, FilterHandle, Predicate, RETAINED_BYTES, criteria,
    patterns,
};
use crate::{
    Error, Response,
    controls::{SCOPE, budget, check, comparison_budget},
    display::Presentation,
    document::Document,
    scalar_input::{self, InputHint},
};
use std::cell::RefCell;
use tablecraft_access::{NativeBrowser, NativeCollationResolver};
use tablecraft_ace::{inspect_database_sort_order, inspect_table_schema};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::{
    EvalError,
    comparison::{Comparison, compare},
    logic::Truth,
};
use tablecraft_schema::ScalarType;

const MAX_FLOAT_LITERAL_BYTES: usize = 512;

impl Document {
    pub(crate) fn describe_field_filter(
        &mut self,
        target: &FieldFilterTarget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        check(cancelled)?;
        if target.document_revision != self.revision {
            return Err(Error::StaleDocument);
        }
        if target.filter_id.is_none() && target.enabled {
            return Err(FilterError::StaleHandle.into());
        }
        let mut native = budget();
        let schema = inspect_table_schema(
            &mut self.source,
            SCOPE,
            &target.table,
            &mut native,
            cancelled,
        )?;
        let position = criteria::field_position(&schema, &target.column, &mut native, cancelled)?;
        let field = &schema.columns()[position];
        criteria::require_field(field)?;
        let kind = field.scalar_type().ok_or(FilterError::UnsupportedField)?;
        let available = operators(kind).ok_or(FilterError::UnsupportedField)?;
        native.charge_allocation(field.name().len() as u64, 1)?;
        let column = field.column()?;
        let pattern_mode = if kind == ScalarType::Text {
            let mode = patterns::mode(&mut self.source, &mut native, cancelled)?;
            let order = inspect_database_sort_order(&mut self.source, &mut native, cancelled)?;
            let comparisons = RefCell::new(comparison_budget());
            let resolver =
                NativeCollationResolver::new_deferred_default(order, &comparisons, cancelled)?;
            let policy = match field.text_sort_order() {
                Some(order) => resolver.resolve_native(order)?,
                None => resolver.default_collation(),
            };
            patterns::qualify(policy)?;
            Some(patterns::describe_mode(mode))
        } else {
            None
        };
        let mut output = Presentation::new(cancelled);
        let copied = FieldFilterTarget {
            document_revision: target.document_revision,
            table: output.text(&target.table)?,
            column: output.text(&target.column)?,
            filter_id: target.filter_id,
            enabled: target.enabled,
        };
        output.allocation(available.len(), size_of::<FieldFilterOperator>())?;
        let mut operators = Vec::new();
        operators.try_reserve_exact(available.len())?;
        operators.extend_from_slice(available);
        Ok(Response::FieldFilterDescription(FieldFilterDescription {
            target: copied,
            input_hint: match kind {
                ScalarType::Byte
                | ScalarType::Int16
                | ScalarType::Int32
                | ScalarType::Int64
                | ScalarType::Currency
                | ScalarType::Decimal => InputHint::Decimal,
                _ => InputHint::from(kind),
            },
            nullable: column.nullable(),
            operators,
            pattern_mode,
        }))
    }

    pub(crate) fn apply_field_filter(
        &mut self,
        target: &FieldFilterTarget,
        operator: FieldFilterOperator,
        inputs: &[String],
        base: Option<&FilterHandle>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        check(cancelled)?;
        if target.document_revision != self.revision {
            return Err(Error::StaleDocument);
        }
        match (base, target.filter_id) {
            (None, None) if !target.enabled => {}
            (Some(base), Some(id)) => {
                self.check_filter(base)?;
                if base.target().filter_id != id
                    || base.target().table != target.table
                    || !base.mode_cached(target.enabled)
                {
                    return Err(FilterError::StaleHandle.into());
                }
            }
            _ => return Err(FilterError::StaleHandle.into()),
        }
        if inputs.len() != operator.input_count() {
            return Err(FilterError::InvalidOperands.into());
        }

        let mut native = budget();
        let schema = inspect_table_schema(
            &mut self.source,
            SCOPE,
            &target.table,
            &mut native,
            cancelled,
        )?;
        let position = criteria::field_position(&schema, &target.column, &mut native, cancelled)?;
        let field = &schema.columns()[position];
        criteria::require_field(field)?;
        let kind = field.scalar_type().ok_or(FilterError::UnsupportedField)?;
        if !operators(kind)
            .ok_or(FilterError::UnsupportedField)?
            .contains(&operator)
        {
            return Err(FilterError::UnsupportedOperator.into());
        }
        native.charge_allocation(field.name().len() as u64, 1)?;
        let declaration = field.column()?.type_descriptor();
        let mut work = ResourceBudget::new(RETAINED_BYTES, 0, FILTER_STEPS);
        let mut predicates = super::super::service::replacement_criteria(
            base,
            target.enabled,
            &target.column,
            &mut work,
            cancelled,
        )?;
        let predicate = entered_predicate(kind, operator, inputs, &mut work, cancelled)?;
        predicates.push(Criterion {
            column: criteria::copy_text(&target.column, &mut work, cancelled)?,
            declaration,
            predicate,
        });
        let mode = patterns::mode_if_needed(&mut self.source, &predicates, &mut native, cancelled)?;
        let order = inspect_database_sort_order(&mut self.source, &mut native, cancelled)?;
        let comparisons = RefCell::new(comparison_budget());
        let resolver =
            NativeCollationResolver::new_deferred_default(order, &comparisons, cancelled)?;
        let mut browser = NativeBrowser::new(&mut self.source, SCOPE, native, cancelled);
        let (ordinals, source_rows) = super::super::scan::membership(
            &mut browser,
            &schema,
            &predicates,
            &resolver,
            mode,
            &mut work,
            cancelled,
        )?;
        let table = criteria::copy_text(&target.table, &mut work, cancelled)?;
        let sort = base
            .map(|base| {
                super::super::sorting::inherited_sort(&base.state.definition, &mut work, cancelled)
            })
            .transpose()?
            .flatten();
        self.publish_view(
            super::super::sorting::ViewBuild {
                table,
                predicates,
                sort,
                ordinals,
                source_rows,
                enabled: true,
                offset: 0,
            },
            &mut work,
            cancelled,
        )
    }
}

fn entered_predicate(
    kind: ScalarType,
    operator: FieldFilterOperator,
    inputs: &[String],
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Predicate, Error> {
    if operator == FieldFilterOperator::IsNull {
        return Ok(Predicate::IsNull);
    }
    if operator == FieldFilterOperator::IsNotNull {
        return Ok(Predicate::IsNotNull);
    }
    let mut output = Presentation::new(cancelled);
    let first = operand(kind, &inputs[0], &mut output)?;
    finite(&first)?;
    if kind == ScalarType::Text {
        let Value::Text(input) = first else {
            return Err(FilterError::UnsupportedField.into());
        };
        return Ok(Predicate::Pattern {
            operator,
            input: criteria::copy_text(&input, work, cancelled)?,
        });
    }
    if matches!(
        operator,
        FieldFilterOperator::Between | FieldFilterOperator::NotBetween
    ) {
        let second = operand(kind, &inputs[1], &mut output)?;
        finite(&second)?;
        return Ok(Predicate::Between {
            first: criteria::copy_value(&first, work, cancelled)?,
            second: criteria::copy_value(&second, work, cancelled)?,
            negated: operator == FieldFilterOperator::NotBetween,
        });
    }
    let value = criteria::copy_value(&first, work, cancelled)?;
    Ok(match operator {
        FieldFilterOperator::Equal => Predicate::Equal(value),
        FieldFilterOperator::NotEqual => Predicate::NotEqual(value),
        FieldFilterOperator::Less => Predicate::Ordered {
            comparison: Comparison::Less,
            expected: value,
        },
        FieldFilterOperator::LessOrEqual => Predicate::Ordered {
            comparison: Comparison::LessOrEqual,
            expected: value,
        },
        FieldFilterOperator::Greater => Predicate::Ordered {
            comparison: Comparison::Greater,
            expected: value,
        },
        FieldFilterOperator::GreaterOrEqual => Predicate::Ordered {
            comparison: Comparison::GreaterOrEqual,
            expected: value,
        },
        _ => return Err(FilterError::UnsupportedOperator.into()),
    })
}

fn finite(value: &Value) -> Result<(), Error> {
    if matches!(value, Value::Single(value) if !value.to_float().is_finite())
        || matches!(value, Value::Double(value) if !value.to_float().is_finite())
    {
        return Err(EvalError::NonFinite.into());
    }
    Ok(())
}

fn operand(kind: ScalarType, input: &str, output: &mut Presentation<'_>) -> Result<Value, Error> {
    if matches!(
        kind,
        ScalarType::Text | ScalarType::DateTime | ScalarType::DateTimeExtended
    ) {
        return scalar_input::parse(kind, input, false, output);
    }
    let exact = scalar_input::exact_numeric(input, output)?;
    if !matches!(kind, ScalarType::Single | ScalarType::Double) {
        return Ok(exact);
    }

    let value = scalar_input::parse(kind, input, false, output)?;
    finite(&value)?;
    output.allocation(1, MAX_FLOAT_LITERAL_BYTES)?;
    let canonical = match &value {
        Value::Single(value) => value.to_float().to_string(),
        Value::Double(value) => value.to_float().to_string(),
        _ => return Err(FilterError::UnsupportedField.into()),
    };
    let represented = scalar_input::exact_numeric(&canonical, output)?;
    if compare(Comparison::Equal, &exact, &represented, None)? != Truth::True {
        return Err(EvalError::PrecisionLoss.into());
    }
    Ok(value)
}
