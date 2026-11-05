//! Conservative SQL-to-grid import with a canonical round-trip proof.
use super::build::{expression, source_name, validate_shape, visit};
use super::{
    BuiltQueryDesign, DesignColumn, DesignExpression, DesignField, DesignJoin, DesignJoinKind,
    DesignSort, DesignSource, DesignTableMetadata, MAX_DESIGN_COLUMNS, MAX_DESIGN_CRITERIA_ROWS,
    QueryDesignDraft, QueryDesignError, build, build_ast,
};
use tablecraft_sql::{
    ParseLimits, parse_select,
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
) -> Result<usize, QueryDesignError> {
    for (index, column) in draft.columns.iter().enumerate() {
        if expression(draft, &column.expression).ok().as_ref() == Some(operand) {
            return Ok(index);
        }
    }
    if draft.columns.len() == MAX_DESIGN_COLUMNS {
        return Err(QueryDesignError::Limit);
    }
    draft.columns.push(DesignColumn {
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
            let column = column_for(draft, operand(&term)?)?;
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

fn imported(select: &Select) -> Result<QueryDesignDraft, QueryDesignError> {
    if select.predicate != SelectPredicate::All
        || !select.group_by.is_empty()
        || select.having.is_some()
        || select.into.is_some()
        || select.into_external.is_some()
        || select.external.is_some()
        || select.owner_access
    {
        return Err(QueryDesignError::Unsupported);
    }
    if let Some(selection) = &select.selection {
        let mut selection = selection.clone();
        visit(&mut selection, &mut |_| Ok(()))?;
    }
    let mut draft = QueryDesignDraft::default();
    for source in &select.from {
        relation(source, &mut draft)?;
    }
    for projection in &select.projection {
        let (value, alias) = match projection {
            Projection::Field { field, alias } => (
                DesignExpression::Expression(Expression::Field(field.clone()).to_string()),
                alias,
            ),
            Projection::Expression { expression, alias } => {
                (DesignExpression::Expression(expression.to_string()), alias)
            }
            Projection::Wildcard(qualifier) => {
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
            expression: value,
            alias: alias.as_ref().map(|alias| alias.as_str().to_owned()),
            show: true,
            sort: None,
            criteria: Vec::new(),
        });
    }
    for sort in &select.order_by {
        let index = column_for(&mut draft, &sort.expression)?;
        if draft.columns[index].sort.is_some() {
            return Err(QueryDesignError::Unsupported);
        }
        draft.columns[index].sort = Some(if sort.descending {
            DesignSort::Descending
        } else {
            DesignSort::Ascending
        });
    }
    if let Some(selection) = &select.selection {
        grid_criteria(&mut draft, selection.clone())?;
    }
    validate_shape(&draft)?;
    for column in &draft.columns {
        if !matches!(column.expression, DesignExpression::AllFields(_)) {
            let mut value = expression(&draft, &column.expression)?;
            visit(&mut value, &mut |_| Ok(()))?;
        }
    }
    Ok(draft)
}

pub fn import_unbound(sql: &str) -> Result<QueryDesignDraft, QueryDesignError> {
    let parsed = parse_select(sql, ParseLimits::default())?;
    imported(parsed.query())
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
    let parsed = parse_select(sql, ParseLimits::default())?;
    let draft = imported(parsed.query())?;
    let built = build(&draft, metadata)?;
    let rebuilt = build_ast(&draft, metadata)?;
    let mut original = parsed.into_query();
    original.from = original.from.into_iter().map(strip_groups).collect();
    // Only transparent parenthesis groups are normalized. Criteria ordering,
    // joins, aliases, sort and every unsupported clause must otherwise survive.
    if tablecraft_sql::query::Query::from_select(original).to_string() != rebuilt.to_string() {
        return Err(QueryDesignError::Unsupported);
    }
    Ok(built)
}
