//! Closed syntax-only dependencies; cached calculations are never evaluated.
use crate::{
    EditError, InspectError, NativeImage, TableSchema, WriteBlocker,
    format::{data_type::INT32, layout::CALCULATED_EXT_MASK},
    header::check,
    inspect_calculation_declaration, read_stored_int32_calculation,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;
use tablecraft_sql::{
    ParseErrorKind, ParseLimits, parse_expression,
    select::{BinaryOperator, Expression, UnaryOperator},
};

const MAX_EXPRESSION_BYTES: usize = 4096;
const MAX_EXPRESSION_TOKENS: usize = 128;
const MAX_EXPRESSION_DEPTH: usize = 16;

pub(super) fn capture(
    image: &NativeImage,
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<usize>, EditError> {
    budget
        .charge_allocation(schema.columns().len() as u64, size_of::<usize>() as u64)
        .map_err(InspectError::from)?;
    let mut dependencies = Vec::new();
    dependencies
        .try_reserve_exact(schema.columns().len())
        .map_err(|_| InspectError::Allocation)?;

    for field in schema.columns() {
        check(budget, cancelled)?;
        if field.extended_flags() & CALCULATED_EXT_MASK == 0 {
            continue;
        }
        if field.native_type() != INT32 {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        let declaration = inspect_calculation_declaration(
            &mut image.reader(),
            image.catalog().catalog().scope(),
            schema.name(),
            field.name(),
            budget,
            cancelled,
        )?;
        let expression = declaration.expression()?;
        if expression.len() > MAX_EXPRESSION_BYTES {
            return Err(InspectError::StructureLimit.into());
        }
        budget
            .charge_allocation(MAX_EXPRESSION_TOKENS as u64, size_of::<Expression>() as u64)
            .map_err(InspectError::from)?;
        budget
            .charge_allocation(expression.len() as u64, 2)
            .map_err(InspectError::from)?;
        budget
            .charge(Resource::Steps, expression.len() as u64)
            .map_err(InspectError::from)?;
        let expression = parse_expression(
            expression,
            ParseLimits {
                max_bytes: MAX_EXPRESSION_BYTES,
                max_tokens: MAX_EXPRESSION_TOKENS,
                max_depth: MAX_EXPRESSION_DEPTH,
            },
        )
        .map_err(|error| {
            if error.kind == ParseErrorKind::Limit {
                EditError::Inspect(InspectError::StructureLimit)
            } else {
                EditError::Unsupported(WriteBlocker::Properties)
            }
        })?;
        visit(&expression, schema, &mut dependencies, budget, cancelled)?;

        // This validates actual native wrappers and carriers without executing
        // the expression or asserting that its cached result is fresh.
        read_stored_int32_calculation(
            &mut image.reader(),
            image.catalog().catalog().scope(),
            schema.name(),
            field.name(),
            budget,
            cancelled,
        )?;
    }
    check(budget, cancelled)?;
    Ok(dependencies)
}

fn visit(
    expression: &Expression,
    schema: &TableSchema,
    dependencies: &mut Vec<usize>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, 1)
        .map_err(InspectError::from)?;
    match expression {
        Expression::Integer(_) => Ok(()),
        Expression::Field(reference) if reference.qualifier.is_none() => {
            let mut number = None;
            for field in schema.columns() {
                check(budget, cancelled)?;
                budget
                    .charge(
                        Resource::Steps,
                        field.name().len().max(reference.name.as_str().len()) as u64,
                    )
                    .map_err(InspectError::from)?;
                if field.name().eq_ignore_ascii_case(reference.name.as_str())
                    && (number.replace(field.number()).is_some()
                        || field.scalar_type() != Some(ScalarType::Int32))
                {
                    return Err(EditError::Unsupported(WriteBlocker::Properties));
                }
            }
            let number = number.ok_or(EditError::Unsupported(WriteBlocker::Properties))?;
            budget
                .charge(Resource::Steps, dependencies.len() as u64)
                .map_err(InspectError::from)?;
            if !dependencies.contains(&number) {
                dependencies.push(number);
            }
            Ok(())
        }
        Expression::Unary {
            operator: UnaryOperator::Plus | UnaryOperator::Minus,
            expression,
        } => visit(expression, schema, dependencies, budget, cancelled),
        Expression::Binary {
            left,
            operator: BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply,
            right,
        } => {
            visit(left, schema, dependencies, budget, cancelled)?;
            visit(right, schema, dependencies, budget, cancelled)
        }
        _ => Err(EditError::Unsupported(WriteBlocker::Properties)),
    }
}
