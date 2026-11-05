use super::{
    BuiltQueryDesign, DesignColumn, DesignExpression, DesignField, DesignJoinKind, DesignSort,
    DesignTableMetadata, MAX_DESIGN_COLUMNS, MAX_DESIGN_CRITERIA_ROWS, MAX_DESIGN_JOINS,
    MAX_DESIGN_METADATA_BYTES, MAX_DESIGN_METADATA_FIELDS, MAX_DESIGN_SOURCES,
    MAX_DESIGN_TEXT_BYTES, QueryDesignDraft, QueryDesignError,
};
use tablecraft_sql::{
    ParseLimits, parse_expression_strict, parse_query,
    query::Query,
    select::{
        BinaryOperator, Expression, FieldReference, FunctionArgument, JoinKind, Name, Projection,
        Select, SortKey, Table, TableSource,
    },
};

pub(super) fn name(value: &str) -> Result<Name, QueryDesignError> {
    Ok(Name::from_contents(value, ParseLimits::default())?)
}

pub(super) fn validate_shape(draft: &QueryDesignDraft) -> Result<(), QueryDesignError> {
    if draft.sources.len() > MAX_DESIGN_SOURCES
        || draft.joins.len() > MAX_DESIGN_JOINS
        || draft.columns.len() > MAX_DESIGN_COLUMNS
    {
        return Err(QueryDesignError::Limit);
    }
    draft_text_bytes(draft)?;
    for (index, source) in draft.sources.iter().enumerate() {
        if let Some(alias) = &source.alias {
            name(alias)?;
        }
        name(&source.table)?;
        if draft.sources[..index].iter().any(|old| {
            old.id == source.id || source_name(old).eq_ignore_ascii_case(source_name(source))
        }) {
            return Err(QueryDesignError::InvalidReference);
        }
    }
    for column in &draft.columns {
        if let Some(alias) = &column.alias {
            name(alias)?;
        }
        if column.criteria.len() > MAX_DESIGN_CRITERIA_ROWS {
            return Err(QueryDesignError::Limit);
        }
    }
    Ok(())
}

/// Combined editable text accounting, also used before UI input admission.
/// Names may still be incomplete while editing; build validates their syntax.
pub fn draft_text_bytes(draft: &QueryDesignDraft) -> Result<usize, QueryDesignError> {
    if draft.sources.len() > MAX_DESIGN_SOURCES
        || draft.joins.len() > MAX_DESIGN_JOINS
        || draft.columns.len() > MAX_DESIGN_COLUMNS
        || draft
            .columns
            .iter()
            .any(|column| column.criteria.len() > MAX_DESIGN_CRITERIA_ROWS)
    {
        return Err(QueryDesignError::Limit);
    }
    let mut bytes = 0_usize;
    let mut charge = |text: &str| -> Result<(), QueryDesignError> {
        bytes = bytes
            .checked_add(text.len())
            .ok_or(QueryDesignError::Limit)?;
        if bytes > MAX_DESIGN_TEXT_BYTES {
            return Err(QueryDesignError::Limit);
        }
        Ok(())
    };
    for source in &draft.sources {
        charge(&source.table)?;
        if let Some(alias) = &source.alias {
            charge(alias)?;
        }
    }
    for join in &draft.joins {
        charge(&join.left.column)?;
        charge(&join.right.column)?;
    }
    for column in &draft.columns {
        match &column.expression {
            DesignExpression::Field(field) => charge(&field.column)?,
            DesignExpression::Expression(expression) => charge(expression)?,
            DesignExpression::AllFields(_) => {}
        }
        if let Some(alias) = &column.alias {
            charge(alias)?;
        }
        for criterion in &column.criteria {
            charge(criterion)?;
        }
    }
    Ok(bytes)
}

pub(super) fn source_name(source: &super::DesignSource) -> &str {
    source.alias.as_deref().unwrap_or(&source.table)
}

pub(super) fn field_reference(
    draft: &QueryDesignDraft,
    field: &DesignField,
) -> Result<FieldReference, QueryDesignError> {
    let source = draft
        .sources
        .iter()
        .find(|source| source.id == field.source_id)
        .ok_or(QueryDesignError::InvalidReference)?;
    Ok(FieldReference {
        qualifier: Some(name(source_name(source))?),
        name: name(&field.column)?,
    })
}

pub(super) fn expression(
    draft: &QueryDesignDraft,
    value: &DesignExpression,
) -> Result<Expression, QueryDesignError> {
    match value {
        DesignExpression::Field(field) => Ok(Expression::Field(field_reference(draft, field)?)),
        DesignExpression::Expression(text) => {
            Ok(parse_expression_strict(text, ParseLimits::default())?)
        }
        DesignExpression::AllFields(_) => Err(QueryDesignError::Unsupported),
    }
}

pub fn expression_text(
    draft: &QueryDesignDraft,
    value: &DesignExpression,
) -> Result<String, QueryDesignError> {
    validate_shape(draft)?;
    match value {
        DesignExpression::AllFields(None) => Ok("*".into()),
        DesignExpression::AllFields(Some(id)) => {
            let source = draft
                .sources
                .iter()
                .find(|source| source.id == *id)
                .ok_or(QueryDesignError::InvalidReference)?;
            Ok(format!("{}.*", name(source_name(source))?))
        }
        _ => Ok(expression(draft, value)?.to_string()),
    }
}

pub(super) fn resolve_field<'a>(
    draft: &'a QueryDesignDraft,
    metadata: &'a [DesignTableMetadata],
    field: &FieldReference,
) -> Result<&'a super::DesignFieldMetadata, QueryDesignError> {
    let mut found = None;
    for source in &draft.sources {
        if field
            .qualifier
            .as_ref()
            .is_some_and(|qualifier| !qualifier.as_str().eq_ignore_ascii_case(source_name(source)))
        {
            continue;
        }
        let table = metadata
            .iter()
            .find(|table| table.table.eq_ignore_ascii_case(&source.table))
            .ok_or(QueryDesignError::InvalidReference)?;
        if let Some(candidate) = table
            .fields
            .iter()
            .find(|candidate| candidate.name.eq_ignore_ascii_case(field.name.as_str()))
        {
            if found.is_some() {
                return Err(QueryDesignError::InvalidReference);
            }
            found = Some(candidate);
        }
    }
    found.ok_or(QueryDesignError::InvalidReference)
}

pub(super) fn visit(
    expression: &mut Expression,
    action: &mut impl FnMut(&mut FieldReference) -> Result<(), QueryDesignError>,
) -> Result<(), QueryDesignError> {
    match expression {
        Expression::Field(field) => action(field),
        Expression::Unary { expression, .. } | Expression::IsNull { expression, .. } => {
            visit(expression, action)
        }
        Expression::Binary { left, right, .. } => {
            visit(left, action)?;
            visit(right, action)
        }
        Expression::Like {
            expression,
            pattern,
            ..
        } => {
            visit(expression, action)?;
            visit(pattern, action)
        }
        Expression::Between {
            expression,
            first_bound,
            second_bound,
            ..
        } => {
            visit(expression, action)?;
            visit(first_bound, action)?;
            visit(second_bound, action)
        }
        Expression::InList {
            expression, values, ..
        } => {
            visit(expression, action)?;
            for value in values {
                visit(value, action)?;
            }
            Ok(())
        }
        Expression::Call {
            qualifier,
            name,
            arguments,
        } => {
            if !qualifier.is_empty()
                || [
                    "COUNT", "SUM", "AVG", "MIN", "MAX", "FIRST", "LAST", "STDEV", "STDEVP", "VAR",
                    "VARP",
                ]
                .iter()
                .any(|aggregate| name.as_str().eq_ignore_ascii_case(aggregate))
            {
                return Err(QueryDesignError::Unsupported);
            }
            for argument in arguments {
                match argument {
                    FunctionArgument::Expression(expression) => visit(expression, action)?,
                    FunctionArgument::Missing => {}
                    FunctionArgument::Wildcard => return Err(QueryDesignError::Unsupported),
                }
            }
            Ok(())
        }
        Expression::Integer(_)
        | Expression::Text(_)
        | Expression::Number(_)
        | Expression::DateLiteral(_)
        | Expression::GuidLiteral(_)
        | Expression::Null
        | Expression::Boolean(_) => Ok(()),
        _ => Err(QueryDesignError::Unsupported),
    }
}

fn validated(
    draft: &QueryDesignDraft,
    metadata: &[DesignTableMetadata],
    mut value: Expression,
) -> Result<Expression, QueryDesignError> {
    visit(&mut value, &mut |field| {
        resolve_field(draft, metadata, field)?;
        Ok(())
    })?;
    Ok(value)
}

fn table(source: &super::DesignSource) -> Result<TableSource, QueryDesignError> {
    Ok(TableSource::Table(Table {
        database: None,
        name: name(&source.table)?,
        alias: source.alias.as_deref().map(name).transpose()?,
    }))
}

fn relations(
    draft: &QueryDesignDraft,
    metadata: &[DesignTableMetadata],
) -> Result<Vec<TableSource>, QueryDesignError> {
    let mut used = vec![false; draft.sources.len()];
    let mut joins = vec![false; draft.joins.len()];
    let mut result = Vec::new();
    for first in 0..draft.sources.len() {
        if used[first] {
            continue;
        }
        let mut members = vec![draft.sources[first].id];
        let mut root = table(&draft.sources[first])?;
        used[first] = true;
        loop {
            let next = draft.joins.iter().enumerate().find(|(index, join)| {
                !joins[*index]
                    && (members.contains(&join.left.source_id)
                        ^ members.contains(&join.right.source_id))
            });
            let Some((index, edge)) = next else {
                break;
            };
            let left_inside = members.contains(&edge.left.source_id);
            let other = if left_inside {
                edge.right.source_id
            } else {
                edge.left.source_id
            };
            let position = draft
                .sources
                .iter()
                .position(|source| source.id == other)
                .ok_or(QueryDesignError::InvalidJoins)?;
            if used[position] {
                return Err(QueryDesignError::InvalidJoins);
            }
            let left = field_reference(draft, &edge.left)?;
            let right = field_reference(draft, &edge.right)?;
            resolve_field(draft, metadata, &left)?;
            resolve_field(draft, metadata, &right)?;
            let condition = Expression::Binary {
                operator: BinaryOperator::Equal,
                left: Box::new(Expression::Field(left)),
                right: Box::new(Expression::Field(right)),
            };
            let leaf = table(&draft.sources[position])?;
            let (left, right) = if left_inside {
                (root, leaf)
            } else {
                (leaf, root)
            };
            root = TableSource::Join {
                left: Box::new(left),
                kind: match edge.kind {
                    DesignJoinKind::Inner => JoinKind::Inner,
                    DesignJoinKind::Left => JoinKind::Left,
                    DesignJoinKind::Right => JoinKind::Right,
                },
                right: Box::new(right),
                condition,
            };
            members.push(other);
            used[position] = true;
            joins[index] = true;
        }
        result.push(root);
    }
    if joins.iter().any(|used| !used) {
        return Err(QueryDesignError::InvalidJoins);
    }
    Ok(result)
}

/// Validate the informational descriptor envelope without granting execution.
pub fn validate_metadata(metadata: &[DesignTableMetadata]) -> Result<(), QueryDesignError> {
    if metadata.len() > MAX_DESIGN_SOURCES
        || metadata
            .iter()
            .any(|table| table.fields.len() > MAX_DESIGN_METADATA_FIELDS)
    {
        return Err(QueryDesignError::Limit);
    }
    let bytes = metadata
        .iter()
        .try_fold(0_usize, |sum, table| {
            table
                .fields
                .iter()
                .try_fold(sum.checked_add(table.table.len())?, |sum, field| {
                    sum.checked_add(field.name.len())
                })
        })
        .ok_or(QueryDesignError::Limit)?;
    if bytes > MAX_DESIGN_METADATA_BYTES {
        return Err(QueryDesignError::Limit);
    }
    for (index, table) in metadata.iter().enumerate() {
        name(&table.table)?;
        if metadata[..index]
            .iter()
            .any(|old| old.table.eq_ignore_ascii_case(&table.table))
        {
            return Err(QueryDesignError::InvalidReference);
        }
        for (position, field) in table.fields.iter().enumerate() {
            name(&field.name)?;
            if table.fields[..position]
                .iter()
                .any(|old| old.name.eq_ignore_ascii_case(&field.name))
            {
                return Err(QueryDesignError::InvalidReference);
            }
        }
    }
    Ok(())
}

pub fn build_ast(
    draft: &QueryDesignDraft,
    metadata: &[DesignTableMetadata],
) -> Result<Query, QueryDesignError> {
    validate_shape(draft)?;
    validate_metadata(metadata)?;
    for source in &draft.sources {
        if metadata
            .iter()
            .filter(|table| table.table.eq_ignore_ascii_case(&source.table))
            .count()
            != 1
        {
            return Err(QueryDesignError::InvalidReference);
        }
    }
    let mut projection = Vec::new();
    let mut order = Vec::new();
    for column in &draft.columns {
        let value = match &column.expression {
            DesignExpression::AllFields(source) => {
                if column.alias.is_some()
                    || column.sort.is_some()
                    || column
                        .criteria
                        .iter()
                        .any(|criterion| !criterion.trim().is_empty())
                {
                    return Err(QueryDesignError::Unsupported);
                }
                let qualifier = source
                    .map(|id| {
                        draft
                            .sources
                            .iter()
                            .find(|source| source.id == id)
                            .ok_or(QueryDesignError::InvalidReference)
                            .and_then(|source| name(source_name(source)))
                    })
                    .transpose()?;
                if column.show {
                    projection.push(Projection::Wildcard(qualifier));
                }
                continue;
            }
            _ => validated(draft, metadata, expression(draft, &column.expression)?)?,
        };
        if column.show {
            projection.push(Projection::Expression {
                expression: value.clone(),
                alias: column.alias.as_deref().map(name).transpose()?,
            });
        }
        if let Some(sort) = column.sort {
            order.push(SortKey {
                expression: value,
                descending: sort == DesignSort::Descending,
            });
        }
    }
    if projection.is_empty() {
        return Err(QueryDesignError::NoProjection);
    }
    let mut select = Select::from_projection(projection, relations(draft, metadata)?);
    select.order_by = order;
    select.selection = super::criteria::selection(draft, metadata)?;
    Ok(Query::from_select(select))
}

pub fn build(
    draft: &QueryDesignDraft,
    metadata: &[DesignTableMetadata],
) -> Result<BuiltQueryDesign, QueryDesignError> {
    let query = build_ast(draft, metadata)?;
    let sql = query.to_string();
    if sql.len() > MAX_DESIGN_TEXT_BYTES {
        return Err(QueryDesignError::Limit);
    }
    // The public parser remains the canonical syntax/structural ceiling.
    parse_query(&sql, ParseLimits::default())?;
    Ok(BuiltQueryDesign {
        draft: draft.clone(),
        sql,
    })
}

pub(super) fn column_expression(
    draft: &QueryDesignDraft,
    metadata: &[DesignTableMetadata],
    column: &DesignColumn,
) -> Result<Expression, QueryDesignError> {
    validated(draft, metadata, expression(draft, &column.expression)?)
}
