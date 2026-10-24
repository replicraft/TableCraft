//! Pure validation calls whose text operands need no ambient conversion policy.
use crate::TableSnapshot;
use tablecraft_schema::ScalarType;
use tablecraft_sql::select::{BinaryOperator, Expression, FunctionArgument, Name};

/// This checks signatures and static operand domains only. The caller must also
/// recurse through every argument with the ordinary field/scope admission check.
pub(super) fn admitted(
    qualifier: &[Name],
    name: &Name,
    arguments: &[FunctionArgument],
    table: &TableSnapshot,
) -> bool {
    if !qualifier.is_empty() {
        return false;
    }
    if name.as_str().eq_ignore_ascii_case("Len") {
        return matches!(arguments, [FunctionArgument::Expression(value)] if text_operand(value, table));
    }
    if name.as_str().eq_ignore_ascii_case("StrComp") {
        return matches!(
            arguments,
            [
                FunctionArgument::Expression(left),
                FunctionArgument::Expression(right),
                FunctionArgument::Expression(Expression::Integer(0)),
            ] if text_operand(left, table) && text_operand(right, table)
        );
    }
    false
}

fn text_operand(expression: &Expression, table: &TableSnapshot) -> bool {
    match expression {
        Expression::Null | Expression::Text(_) => true,
        Expression::Field(reference) => table.columns().iter().any(|column| {
            column.name().eq_ignore_ascii_case(reference.name.as_str())
                && column.data_type() == ScalarType::Text
        }),
        Expression::Binary {
            left,
            operator: BinaryOperator::Add | BinaryOperator::Concatenate,
            right,
        } => text_operand(left, table) && text_operand(right, table),
        _ => false,
    }
}
