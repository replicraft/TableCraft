//! Native grid grouping: fields on one row AND, alternative rows OR.
use super::build::{column_expression, resolve_field, visit};
use super::{DesignTableMetadata, QueryDesignDraft, QueryDesignError};
use tablecraft_schema::ScalarType;
use tablecraft_sql::{
    ParseLimits, parse_expression_strict, parse_field_rule,
    select::{BinaryOperator, Expression, UnaryOperator},
};

pub(super) fn combine(operator: BinaryOperator, values: Vec<Expression>) -> Option<Expression> {
    values.into_iter().reduce(|left, right| Expression::Binary {
        operator,
        left: Box::new(left),
        right: Box::new(right),
    })
}

fn predicate(value: &Expression) -> bool {
    matches!(
        value,
        Expression::Binary {
            operator: BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual
                | BinaryOperator::And
                | BinaryOperator::Or
                | BinaryOperator::Xor
                | BinaryOperator::Equivalence
                | BinaryOperator::Implication,
            ..
        } | Expression::Like { .. }
            | Expression::Between { .. }
            | Expression::InList { .. }
            | Expression::IsNull { .. }
            | Expression::Unary {
                operator: UnaryOperator::Not,
                ..
            }
    )
}

fn bare_text(text: &str) -> bool {
    if text.is_empty()
        || text.starts_with(['\'', '"', '#', '[', '(', '<', '>', '='])
        || text.contains(['[', ']', '(', ')', ';', '<', '>', '=', '*', '?', '%'])
    {
        return false;
    }
    if text.split_whitespace().any(|word| {
        [
            "AND", "OR", "NOT", "LIKE", "IN", "BETWEEN", "IS", "NULL", "TRUE", "FALSE",
        ]
        .iter()
        .any(|keyword| word.eq_ignore_ascii_case(keyword))
    }) {
        return false;
    }
    matches!(
        parse_expression_strict(text, ParseLimits::default()),
        Ok(Expression::Field(_) | Expression::Integer(_) | Expression::Number(_)) | Err(_)
    )
}

fn criterion(
    draft: &QueryDesignDraft,
    metadata: &[DesignTableMetadata],
    current: &Expression,
    text: &str,
) -> Result<Expression, QueryDesignError> {
    let text = text.trim();
    let mut value = if let Expression::Field(field) = current {
        let kind = resolve_field(draft, metadata, field)?.scalar_type;
        if kind == Some(ScalarType::Text) && bare_text(text) {
            Expression::Text(text.to_owned())
        } else {
            let mut parsed = parse_field_rule(text, field.name.as_str(), ParseLimits::default())?;
            // Qualify the contextual column as one owned identifier. Explicitly
            // qualified references to other columns retain their own identity.
            visit(&mut parsed, &mut |reference| {
                if reference.qualifier.is_none()
                    && reference
                        .name
                        .as_str()
                        .eq_ignore_ascii_case(field.name.as_str())
                {
                    // Each contextual predicate field is replaced with an owned qualified
                    // reference while the original current field remains the template for other
                    // occurrences.
                    *reference = field.clone();
                }
                Ok(())
            })?;
            parsed
        }
    } else {
        // A computed grid operand accepts explicit complete predicate syntax or
        // a scalar operand; bare comparisons need a declared contextual field.
        parse_expression_strict(text, ParseLimits::default())?
    };
    visit(&mut value, &mut |reference| {
        resolve_field(draft, metadata, reference)?;
        Ok(())
    })?;
    if matches!(value, Expression::Null) {
        return Err(QueryDesignError::Unsupported);
    }
    Ok(if predicate(&value) {
        value
    } else {
        Expression::Binary {
            operator: BinaryOperator::Equal,
            // A scalar criterion wraps an owned current-operand AST in an equality predicate
            // while retaining the borrowed grid expression for other criteria rows.
            left: Box::new(current.clone()),
            right: Box::new(value),
        }
    })
}

pub(super) fn selection(
    draft: &QueryDesignDraft,
    metadata: &[DesignTableMetadata],
) -> Result<Option<Expression>, QueryDesignError> {
    let count = draft
        .columns
        .iter()
        .map(|column| column.criteria.len())
        .max()
        .unwrap_or(0);
    let mut alternatives = Vec::new();
    for row in 0..count {
        let mut terms = Vec::new();
        for column in &draft.columns {
            let Some(text) = column
                .criteria
                .get(row)
                .filter(|text| !text.trim().is_empty())
            else {
                continue;
            };
            let current = column_expression(draft, metadata, column)?;
            terms.push(criterion(draft, metadata, &current, text)?);
        }
        if let Some(row) = combine(BinaryOperator::And, terms) {
            alternatives.push(row);
        }
    }
    Ok(combine(BinaryOperator::Or, alternatives))
}
