//! Query-grid grouping roles lower into ordinary SELECT/GROUP BY/HAVING syntax.
use super::{
    DesignColumn, DesignExpression, DesignSort, DesignTableMetadata, DesignTotal, QueryDesignDraft,
    QueryDesignError,
    build::{expression, name, relations, validated_with_aggregates, visit},
    criteria::{combine, criterion},
};
use tablecraft_sql::{
    query::Query,
    select::{BinaryOperator, Expression, FunctionArgument, Projection, Select, SortKey},
};

/// Check the bounded editor's OFF transition without changing the draft.
/// Nondefault roles and explicit aggregates must be edited deliberately first.
/// This is an editor qualification boundary, not native toggle-off parity.
///
/// # Errors
/// Refuses nondefault roles, aggregate expressions and malformed field syntax.
pub fn validate_totals_off(draft: &QueryDesignDraft) -> Result<(), QueryDesignError> {
    for column in &draft.columns {
        if column.total != DesignTotal::GroupBy {
            return Err(QueryDesignError::Unsupported);
        }
        if !matches!(column.expression, DesignExpression::AllFields(_)) {
            let mut value = expression(draft, &column.expression)?;
            visit(&mut value, &mut |_| Ok(()))?;
        }
    }
    Ok(())
}

pub(super) fn aggregate(total: DesignTotal) -> Option<&'static str> {
    Some(match total {
        DesignTotal::Sum => "Sum",
        DesignTotal::Avg => "Avg",
        DesignTotal::Min => "Min",
        DesignTotal::Max => "Max",
        DesignTotal::Count => "Count",
        DesignTotal::First => "First",
        DesignTotal::Last => "Last",
        _ => return None,
    })
}

pub(super) fn operand(
    draft: &QueryDesignDraft,
    metadata: &[DesignTableMetadata],
    column: &DesignColumn,
) -> Result<Expression, QueryDesignError> {
    let value = expression(draft, &column.expression)?;
    let value = validated_with_aggregates(
        draft,
        metadata,
        value,
        column.total == DesignTotal::Expression,
    )?;
    Ok(match aggregate(column.total) {
        Some(function) => Expression::Call {
            qualifier: Vec::new(),
            name: name(function)?,
            arguments: vec![FunctionArgument::Expression(value)],
        },
        None => value,
    })
}

fn rows(values: Vec<Vec<Expression>>) -> Option<Expression> {
    combine(
        BinaryOperator::Or,
        values
            .into_iter()
            .filter_map(|row| combine(BinaryOperator::And, row))
            .collect(),
    )
}

pub(super) fn build_ast(
    draft: &QueryDesignDraft,
    metadata: &[DesignTableMetadata],
) -> Result<Query, QueryDesignError> {
    let count = draft
        .columns
        .iter()
        .map(|column| column.criteria.len())
        .max()
        .unwrap_or(0);
    let mut before: Vec<Vec<Expression>> = (0..count).map(|_| Vec::new()).collect();
    let mut after: Vec<Vec<Expression>> = (0..count).map(|_| Vec::new()).collect();
    let mut projection = Vec::new();
    let mut group_by = Vec::new();
    let mut order_by = Vec::new();
    for column in &draft.columns {
        if column.total == DesignTotal::Where && (column.show || column.sort.is_some()) {
            return Err(QueryDesignError::Unsupported);
        }
        let value = operand(draft, metadata, column)?;
        let target = if column.total == DesignTotal::Where {
            &mut before
        } else {
            &mut after
        };
        for (row, text) in column.criteria.iter().enumerate() {
            if !text.trim().is_empty() {
                target[row].push(criterion(
                    draft,
                    metadata,
                    &value,
                    text,
                    column.total != DesignTotal::Where,
                )?);
            }
        }
        if column.total == DesignTotal::Where {
            continue;
        }
        if column.total == DesignTotal::GroupBy {
            if !column.show && column.sort.is_none() {
                group_by.push(value);
                continue;
            }
            // NO_CLONE: GROUP BY and the visible/sorted output retain separate
            // owned AST nodes; the public query AST has no borrowed child links.
            group_by.push(value.clone());
        }
        let alias = column.alias.as_deref().map(name).transpose()?;
        match column.sort {
            Some(sort) => {
                if column.show {
                    // NO_CLONE: Projection and sort are independent owned AST
                    // positions; move the original operand into the sort key.
                    projection.push(Projection::Expression {
                        expression: value.clone(),
                        alias,
                    });
                }
                order_by.push(SortKey {
                    expression: value,
                    descending: sort == DesignSort::Descending,
                });
            }
            None if column.show => projection.push(Projection::Expression {
                expression: value,
                alias,
            }),
            None => {}
        }
    }
    if projection.is_empty() {
        return Err(QueryDesignError::NoProjection);
    }
    let mixed = before.iter().any(|row| !row.is_empty()) && after.iter().any(|row| !row.is_empty());
    if mixed
        && before
            .iter()
            .zip(&after)
            .filter(|(a, b)| !a.is_empty() || !b.is_empty())
            .count()
            > 1
    {
        // Factoring cross-phase alternatives would change both record admission
        // and grouping semantics. Preserve the draft and refuse that composition.
        return Err(QueryDesignError::Unsupported);
    }
    let mut select = Select::from_projection(projection, relations(draft, metadata)?);
    select.selection = rows(before);
    select.group_by = group_by;
    select.having = rows(after);
    select.order_by = order_by;
    let mut query = Query::from_select(select);
    // NO_CLONE: The emitted owned query and editable draft need independent
    // declaration namespaces; borrowed parameters cannot outlive this call.
    query.parameters = draft.parameters.clone();
    Ok(query)
}
