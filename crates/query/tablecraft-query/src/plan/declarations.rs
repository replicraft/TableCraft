//! Pure declaration categories shared by binding and execution; no evaluation.
pub(crate) mod functions;
use tablecraft_expressions::numeric::OperandDeclaration;
use tablecraft_sql::select::Expression;

/// Classify a bound literal's syntax, never an evaluated nullable value.
/// NULL is a Variant literal; the other admitted literal forms keep their
/// existing scalar category.
pub(crate) fn literal_declaration(expression: &Expression) -> OperandDeclaration {
    match expression {
        Expression::Null => OperandDeclaration::Variant,
        _ => OperandDeclaration::Scalar,
    }
}

pub(crate) fn combined(
    left: Option<OperandDeclaration>,
    right: Option<OperandDeclaration>,
) -> Option<OperandDeclaration> {
    match (left, right) {
        (Some(OperandDeclaration::Variant), _) | (_, Some(OperandDeclaration::Variant)) => {
            Some(OperandDeclaration::Variant)
        }
        (Some(OperandDeclaration::Scalar), Some(OperandDeclaration::Scalar)) => {
            Some(OperandDeclaration::Scalar)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_syntax_is_variant_while_other_literal_categories_stay_scalar() {
        assert_eq!(
            literal_declaration(&Expression::Null),
            OperandDeclaration::Variant
        );
        for expression in [
            Expression::Integer(0),
            Expression::Number("1.5".into()),
            Expression::Boolean(false),
            Expression::Text("NULL".into()),
        ] {
            assert_eq!(literal_declaration(&expression), OperandDeclaration::Scalar);
        }
    }
}
