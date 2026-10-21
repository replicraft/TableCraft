//! Guarded predicates in their native per-join precedence order.
use super::{Context, E};
use tablecraft_sql::select::{BinaryOperator, Expression};

pub(super) struct Condition<'a> {
    pub text: &'a str,
    pub parsed: Expression,
}
pub(super) fn matches(
    parts: &[Condition<'_>],
    mut actual: &Expression,
    context: &mut Context<'_>,
) -> Result<bool, E> {
    let Some((last, earlier)) = parts.split_last() else {
        return Err(E::InvalidMetadata);
    };
    // SQL text uses most-recent predicate first, with left-associated AND.
    // Peel the oldest right operand first to verify the entire guarded tree.
    for condition in earlier {
        context.check()?;
        let Expression::Binary {
            left,
            operator: BinaryOperator::And,
            right,
        } = actual
        else {
            return Ok(false);
        };
        if **right != condition.parsed {
            return Ok(false);
        }
        actual = left;
    }
    context.check()?;
    Ok(*actual == last.parsed)
}
