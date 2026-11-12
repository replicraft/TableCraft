//! Conservative SQL-to-grid import with a canonical round-trip proof.
mod roles;
use super::build::{expression, source_name, validate_shape, visit_with_aggregates};
use super::{
    BuiltQueryDesign, DesignColumn, DesignExpression, DesignField, DesignJoin, DesignJoinKind,
    DesignSort, DesignSource, DesignTableMetadata, DesignTotal, MAX_DESIGN_COLUMNS,
    MAX_DESIGN_CRITERIA_ROWS, QueryDesignDraft, QueryDesignError, build, build_ast,
};
use tablecraft_sql::{
    ParseLimits, parse_query,
    query::{Parameter, Query, ReadBody, Statement},
    select::{
        BinaryOperator, Expression, FieldReference, JoinKind, Projection, Select, SelectPredicate,
        TableSource,
    },
};

fn reference(
    draft: &QueryDesignDraft,
    field: &FieldReference,
) -> Result<DesignField, QueryDesignError> {
    let qualifier = field
        .qualifier
        .as_ref()
        .ok_or(QueryDesignError::Unsupported)?;
    let source = draft
        .sources
        .iter()
        .find(|source| source_name(source).eq_ignore_ascii_case(qualifier.as_str()))
        .ok_or(QueryDesignError::InvalidReference)?;
    Ok(DesignField {
        source_id: source.id,
        column: field.name.as_str().to_owned(),
    })
}

fn relation(source: &TableSource, draft: &mut QueryDesignDraft) -> Result<(), QueryDesignError> {
    match source {
        TableSource::Table(table) if table.database.is_none() => {
            draft.sources.push(DesignSource {
                id: draft.sources.len() as u64 + 1,
                table: table.name.as_str().to_owned(),
                alias: table.alias.as_ref().map(|alias| alias.as_str().to_owned()),
            });
            Ok(())
        }
        TableSource::Group {
            source,
            alias: None,
        } => relation(source, draft),
        TableSource::Join {
            left,
            kind,
            right,
            condition,
        } => {
            relation(left, draft)?;
            relation(right, draft)?;
            let Expression::Binary {
                operator: BinaryOperator::Equal,
                left,
                right,
            } = condition
            else {
                return Err(QueryDesignError::Unsupported);
            };
            let (Expression::Field(left), Expression::Field(right)) = (&**left, &**right) else {
                return Err(QueryDesignError::Unsupported);
            };
            draft.joins.push(DesignJoin {
                left: reference(draft, left)?,
                right: reference(draft, right)?,
                kind: match kind {
                    JoinKind::Inner => DesignJoinKind::Inner,
                    JoinKind::Left => DesignJoinKind::Left,
                    JoinKind::Right => DesignJoinKind::Right,
                },
            });
            Ok(())
        }
        _ => Err(QueryDesignError::Unsupported),
    }
}

fn flatten(value: Expression, operator: BinaryOperator, output: &mut Vec<Expression>) {
    match value {
        Expression::Binary {
            operator: actual,
            left,
            right,
        } if actual == operator => {
            flatten(*left, operator, output);
            flatten(*right, operator, output);
        }
        other => output.push(other),
    }
}

fn operand(value: &Expression) -> Result<&Expression, QueryDesignError> {
    match value {
        Expression::Binary {
            operator:
                BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual,
            left,
            ..
        } => Ok(left),
        Expression::Like { expression, .. }
        | Expression::Between { expression, .. }
        | Expression::InList { expression, .. }
        | Expression::IsNull { expression, .. } => Ok(expression),
        _ => Err(QueryDesignError::Unsupported),
    }
}

fn column_for(
    draft: &mut QueryDesignDraft,
    operand: &Expression,
    role: DesignTotal,
) -> Result<usize, QueryDesignError> {
    for (index, column) in draft.columns.iter().enumerate() {
        if matches!(column.expression, DesignExpression::AllFields(_)) {
            continue;
        }
        let current = roles::operand(draft, column)?;
        if (!draft.totals
            || column.total == role
            || (role == DesignTotal::Expression && column.total != DesignTotal::Where))
            && &current == operand
        {
            return Ok(index);
        }
    }
    if draft.columns.len() == MAX_DESIGN_COLUMNS {
        return Err(QueryDesignError::Limit);
    }
    draft.columns.push(DesignColumn {
        total: role,
        expression: DesignExpression::Expression(operand.to_string()),
        alias: None,
        show: false,
        sort: None,
        criteria: Vec::new(),
    });
    Ok(draft.columns.len() - 1)
}

fn grid_criteria(
    draft: &mut QueryDesignDraft,
    selection: Expression,
    role: DesignTotal,
) -> Result<(), QueryDesignError> {
    let mut alternatives = Vec::new();
    flatten(selection, BinaryOperator::Or, &mut alternatives);
    if alternatives.len() > MAX_DESIGN_CRITERIA_ROWS {
        return Err(QueryDesignError::Limit);
    }
    for (row, alternative) in alternatives.into_iter().enumerate() {
        let mut terms = Vec::new();
        flatten(alternative, BinaryOperator::And, &mut terms);
        for term in terms {
            let column = column_for(draft, operand(&term)?, role)?;
            let criteria = &mut draft.columns[column].criteria;
            criteria.resize_with(row + 1, String::new);
            let old = &mut criteria[row];
            *old = if old.is_empty() {
                term.to_string()
            } else {
                format!("({old}) AND ({term})")
            };
        }
    }
    Ok(())
}

fn imported(
    mut select: Select,
    parameters: Vec<Parameter>,
) -> Result<QueryDesignDraft, QueryDesignError> {
    if select.predicate != SelectPredicate::All
        || select.into.is_some()
        || select.into_external.is_some()
        || select.external.is_some()
        || select.owner_access
    {
        return Err(QueryDesignError::Unsupported);
    }
    let totals = !select.group_by.is_empty()
        || select.having.is_some()
        || select.projection.iter().any(|projection| match projection {
            Projection::Expression { expression, .. } => roles::contains_aggregate(expression),
            _ => false,
        })
        || select
            .order_by
            .iter()
            .any(|sort| roles::contains_aggregate(&sort.expression));
    let mut draft = QueryDesignDraft {
        totals,
        parameters,
        ..QueryDesignDraft::default()
    };
    for source in &select.from {
        relation(source, &mut draft)?;
    }
    for projection in std::mem::take(&mut select.projection) {
        let (value, total, alias) = match projection {
            Projection::Field { field, alias } => {
                let (value, total) =
                    roles::imported(Expression::Field(field), totals, &select.group_by);
                (value, total, alias)
            }
            Projection::Expression { expression, alias } => {
                let (value, total) = roles::imported(expression, totals, &select.group_by);
                (value, total, alias)
            }
            Projection::Wildcard(qualifier) => {
                if totals {
                    return Err(QueryDesignError::Unsupported);
                }
                let id = qualifier
                    .as_ref()
                    .map(|qualifier| {
                        draft
                            .sources
                            .iter()
                            .find(|source| {
                                source_name(source).eq_ignore_ascii_case(qualifier.as_str())
                            })
                            .map(|source| source.id)
                            .ok_or(QueryDesignError::InvalidReference)
                    })
                    .transpose()?;
                draft.columns.push(DesignColumn {
                    total: DesignTotal::GroupBy,
                    expression: DesignExpression::AllFields(id),
                    alias: None,
                    show: true,
                    sort: None,
                    criteria: Vec::new(),
                });
                continue;
            }
        };
        draft.columns.push(DesignColumn {
            total,
            expression: value,
            alias: alias.as_ref().map(|alias| alias.as_str().to_owned()),
            show: true,
            sort: None,
            criteria: Vec::new(),
        });
    }
    for key in &select.group_by {
        column_for(&mut draft, key, DesignTotal::GroupBy)?;
    }
    for sort in &select.order_by {
        let index = column_for(&mut draft, &sort.expression, DesignTotal::Expression)?;
        if draft.columns[index].sort.is_some() {
            return Err(QueryDesignError::Unsupported);
        }
        draft.columns[index].sort = Some(if sort.descending {
            DesignSort::Descending
        } else {
            DesignSort::Ascending
        });
    }
    if let Some(selection) = select.selection.take() {
        let role = if totals {
            DesignTotal::Where
        } else {
            DesignTotal::GroupBy
        };
        grid_criteria(&mut draft, selection, role)?;
    }
    if let Some(having) = select.having.take() {
        grid_criteria(&mut draft, having, DesignTotal::Expression)?;
    }
    validate_shape(&draft)?;
    for column in &draft.columns {
        if !matches!(column.expression, DesignExpression::AllFields(_)) {
            let mut value = expression(&draft, &column.expression)?;
            visit_with_aggregates(
                &mut value,
                totals && column.total == DesignTotal::Expression,
                &mut |_| Ok(()),
            )?;
        }
    }
    Ok(draft)
}

fn into_select(query: Query) -> Result<(Select, Vec<Parameter>), QueryDesignError> {
    let Statement::Read(read) = query.statement else {
        return Err(QueryDesignError::Unsupported);
    };
    let ReadBody::Select(mut select) = read.body else {
        return Err(QueryDesignError::Unsupported);
    };
    if !select.order_by.is_empty() && !read.order_by.is_empty() {
        return Err(QueryDesignError::Unsupported);
    }
    select.order_by.extend(read.order_by);
    select.owner_access |= read.owner_access;
    Ok((*select, query.parameters))
}

pub fn import_unbound(sql: &str) -> Result<QueryDesignDraft, QueryDesignError> {
    let parsed = parse_query(sql, ParseLimits::default())?.into_query();
    let (mut select, parameters) = into_select(parsed)?;
    roles::normalize(&mut select)?;
    imported(select, parameters)
}

fn strip_groups(source: TableSource) -> TableSource {
    match source {
        TableSource::Group {
            source,
            alias: None,
        } => strip_groups(*source),
        TableSource::Join {
            left,
            kind,
            right,
            condition,
        } => TableSource::Join {
            left: Box::new(strip_groups(*left)),
            kind,
            right: Box::new(strip_groups(*right)),
            condition,
        },
        other => other,
    }
}

pub fn import(
    sql: &str,
    metadata: &[DesignTableMetadata],
) -> Result<BuiltQueryDesign, QueryDesignError> {
    let mut original = parse_query(sql, ParseLimits::default())?.into_query();
    let Statement::Read(read) = &mut original.statement else {
        return Err(QueryDesignError::Unsupported);
    };
    let ReadBody::Select(select) = &mut read.body else {
        return Err(QueryDesignError::Unsupported);
    };
    select.from = std::mem::take(&mut select.from)
        .into_iter()
        .map(strip_groups)
        .collect();
    let (mut select, parameters) = into_select(original)?;
    roles::normalize(&mut select)?;
    let mut original = Query::from_select(select);
    original.parameters = parameters;
    let canonical = original.to_string();
    let (select, parameters) = into_select(original)?;
    let draft = imported(select, parameters)?;
    let built = build(&draft, metadata)?;
    let rebuilt = build_ast(&draft, metadata)?;
    // Projection/group/filter/order and declarations must all survive exactly.
    // No SQL-equivalence rewrite or cross-phase OR factoring is inferred.
    if canonical != rebuilt.to_string() {
        return Err(QueryDesignError::Unsupported);
    }
    Ok(built)
}
