//! Native grid grouping: fields on one row AND, alternative rows OR.
mod context;
use super::build::{column_expression, resolve_symbol, visit_with_aggregates};
use super::{DesignTableMetadata, QueryDesignDraft, QueryDesignError};
use tablecraft_schema::ScalarType;
use tablecraft_sql::{
    ParseErrorKind, ParseLimits, parse_expression_strict, parse_field_rule,
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

fn bare_text(text: &str) -> Result<bool, QueryDesignError> {
    if text.is_empty()
        || text.starts_with(['\'', '\"', '#', '[', '(', '<', '>', '='])
        || text.contains(['[', ']', '(', ')', ';', '<', '>', '=', '*', '?', '%'])
    {
        return Ok(false);
    }
    if text.split_whitespace().any(|word| {
        [
            "AND", "OR", "NOT", "LIKE", "IN", "BETWEEN", "IS", "NULL", "TRUE", "FALSE",
        ]
        .iter()
        .any(|keyword| word.eq_ignore_ascii_case(keyword))
    }) {
        return Ok(false);
    }
    match parse_expression_strict(text, ParseLimits::default()) {
        Ok(Expression::Field(_) | Expression::Integer(_) | Expression::Number(_)) => Ok(true),
        Ok(_) => Ok(false),
        Err(error) => match error.kind {
            ParseErrorKind::Limit => Err(error.into()),
            ParseErrorKind::Syntax | ParseErrorKind::Unsupported => Ok(true),
        },
    }
}

fn criterion_word(text: &str) -> (&str, &str) {
    let end = text
        .find(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .unwrap_or(text.len());
    (&text[..end], text[end..].trim_start())
}

fn shorthand(text: &str) -> bool {
    if text.starts_with(['<', '>', '=']) {
        return true;
    }
    let (first, rest) = criterion_word(text);
    if ["LIKE", "BETWEEN", "IN", "IS"]
        .iter()
        .any(|word| first.eq_ignore_ascii_case(word))
    {
        return true;
    }
    if first.eq_ignore_ascii_case("NOT") {
        let (second, _) = criterion_word(rest);
        return ["LIKE", "BETWEEN", "IN"]
            .iter()
            .any(|word| second.eq_ignore_ascii_case(word));
    }
    false
}

pub(super) fn criterion(
    draft: &QueryDesignDraft,
    metadata: &[DesignTableMetadata],
    current: &Expression,
    text: &str,
    aggregates: bool,
) -> Result<Expression, QueryDesignError> {
    let text = text.trim();
    let mut value = if let Expression::Field(field) = current {
        let kind = resolve_symbol(draft, metadata, field)?.and_then(|field| field.scalar_type);
        if kind == Some(ScalarType::Text) && bare_text(text)? {
            Expression::Text(text.to_owned())
        } else {
            let mut parsed = parse_field_rule(text, field.name.as_str(), ParseLimits::default())?;
            // Qualify the contextual column as one owned identifier. Explicitly
            // qualified references to other columns retain their own identity.
            visit_with_aggregates(&mut parsed, aggregates, &mut |reference| {
                if reference.qualifier.is_none()
                    && reference
                        .name
                        .as_str()
                        .eq_ignore_ascii_case(field.name.as_str())
                {
                    // NO_CLONE: Each contextual predicate field is replaced with an owned
                    // qualified reference while the original current field remains
                    // the template for other occurrences.
                    *reference = field.clone();
                }
                Ok(())
            })?;
            parsed
        }
    } else if shorthand(text) {
        context::parse(text, current)?
    } else {
        parse_expression_strict(text, ParseLimits::default())?
    };
    visit_with_aggregates(&mut value, aggregates, &mut |reference| {
        resolve_symbol(draft, metadata, reference)?;
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
            // NO_CLONE: A scalar criterion wraps an owned current-operand AST in an equality
            // predicate while retaining the borrowed grid expression for other
            // criteria rows.
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
            terms.push(criterion(draft, metadata, &current, text, false)?);
        }
        if let Some(row) = combine(BinaryOperator::And, terms) {
            alternatives.push(row);
        }
    }
    Ok(combine(BinaryOperator::Or, alternatives))
}
