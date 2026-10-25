//! Checked owned query construction; stored formula text is never interpolated.
use super::{CalculationView, MAX_NAME_BYTES, MAX_VIEW_COLUMNS, RecomputedNativeSource, View};
use crate::source::{CalculationError, NativeSource};
use tablecraft_ace::{NativeReadSource, NativeTableStorage};
use tablecraft_core::limits::Resource;
use tablecraft_query::QueryError;
use tablecraft_sql::{
    ParseLimits, parse_query,
    query::{Query, ReadBody, ReadQuery, Statement},
    select::{Expression, Projection, Select},
};

struct Selected {
    name: String,
    expression: Option<Expression>,
}

pub(super) fn prepare<R: NativeReadSource>(
    source: &NativeSource<'_, R>,
    request: CalculationView<'_>,
    limits: ParseLimits,
) -> Result<View, CalculationError> {
    if request.columns.is_empty() || request.columns.len() > MAX_VIEW_COLUMNS {
        return Err(QueryError::StructureLimit.into());
    }
    identifier(request.alias)?;
    let name = copy(source, request.alias)?;
    for table in &source.entries {
        RecomputedNativeSource::tick(source)?;
        if table.name.eq_ignore_ascii_case(&name) {
            return Err(QueryError::AmbiguousName.into());
        }
    }
    for query in &source.queries {
        RecomputedNativeSource::tick(source)?;
        if query.has_name(&name) {
            return Err(QueryError::AmbiguousName.into());
        }
    }

    let entry = source.entry(request.table)?;
    if entry.origin.storage != NativeTableStorage::Local {
        return Err(CalculationError::Unsupported);
    }
    identifier(&entry.name)?;
    let columns = source.schema_columns(&entry.name)?;
    charge(source, request.columns.len(), size_of::<Selected>())?;
    let mut selected = Vec::new();
    selected
        .try_reserve_exact(request.columns.len())
        .map_err(|_| QueryError::Allocation)?;

    for requested in request.columns {
        RecomputedNativeSource::tick(source)?;
        identifier(requested)?;
        let mut ordinary = None;
        for (index, column) in columns.iter().enumerate() {
            RecomputedNativeSource::tick(source)?;
            if column.name().eq_ignore_ascii_case(requested) {
                if source.is_stored_calculation(&entry.name, index)? == Some(false) {
                    ordinary = Some(column.name());
                }
                break;
            }
        }
        let value = if let Some(ordinary) = ordinary {
            Selected {
                name: copy(source, ordinary)?,
                expression: None,
            }
        } else {
            let definition = source.calculation_definition(&entry.name, requested, limits)?;
            let canonical = copy(source, definition.declaration.column().name())?;
            let Statement::Read(read) = definition.query.statement else {
                return Err(CalculationError::Unsupported);
            };
            let ReadBody::Select(mut select) = read.body else {
                return Err(CalculationError::Unsupported);
            };
            let Some(Projection::Expression { expression, .. }) = select.projection.pop() else {
                return Err(CalculationError::Unsupported);
            };
            Selected {
                name: canonical,
                expression: Some(expression),
            }
        };
        for previous in &selected {
            RecomputedNativeSource::tick(source)?;
            let previous: &Selected = previous;
            if previous.name.eq_ignore_ascii_case(&value.name) {
                return Err(QueryError::AmbiguousName.into());
            }
        }
        selected.push(value);
    }

    let query = query(source, &entry.name, selected, limits)?;
    Ok(View { name, query })
}

fn identifier(name: &str) -> Result<(), CalculationError> {
    if name.len() > MAX_NAME_BYTES {
        return Err(QueryError::StructureLimit.into());
    }
    if name.is_empty() || name.contains(']') || name.chars().any(char::is_control) {
        return Err(CalculationError::Unsupported);
    }
    Ok(())
}

fn copy<R: NativeReadSource>(
    source: &NativeSource<'_, R>,
    value: &str,
) -> Result<String, CalculationError> {
    charge(source, value.len(), size_of::<u8>())?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(value.len())
        .map_err(|_| QueryError::Allocation)?;
    owned.push_str(value);
    Ok(owned)
}

fn charge<R: NativeReadSource>(
    source: &NativeSource<'_, R>,
    count: usize,
    width: usize,
) -> Result<(), CalculationError> {
    RecomputedNativeSource::tick(source)?;
    source
        .budget
        .try_borrow_mut()
        .map_err(|_| QueryError::Source)?
        .charge_allocation(count as u64, width as u64)
        .map_err(QueryError::from)?;
    Ok(())
}

fn query<R: NativeReadSource>(
    source: &NativeSource<'_, R>,
    table: &str,
    selected: Vec<Selected>,
    limits: ParseLimits,
) -> Result<Query, CalculationError> {
    // Every selector is one checked bracket identifier. Only these identifiers
    // enter the skeleton; each formula remains a separately validated owned AST.
    let mut length = "SELECT "
        .len()
        .checked_add(" FROM []".len())
        .and_then(|value| value.checked_add(table.len()))
        .ok_or(QueryError::StructureLimit)?;
    for value in &selected {
        RecomputedNativeSource::tick(source)?;
        identifier(&value.name)?;
        length = length
            .checked_add(value.name.len())
            .and_then(|value| value.checked_add("[],".len()))
            .ok_or(QueryError::StructureLimit)?;
    }
    charge(source, length, size_of::<u8>())?;
    charge(
        source,
        1,
        size_of::<Query>() + size_of::<ReadQuery>() + size_of::<Select>(),
    )?;
    charge(
        source,
        selected.len(),
        size_of::<Projection>() + size_of::<Expression>(),
    )?;
    // Retained identifier copies (including aliases for replaced formulas) and
    // the FROM model are bounded separately from the temporary parser skeleton.
    charge(source, length, size_of::<char>())?;
    let mut skeleton = String::new();
    skeleton
        .try_reserve_exact(length)
        .map_err(|_| QueryError::Allocation)?;
    skeleton.push_str("SELECT ");
    for (index, value) in selected.iter().enumerate() {
        RecomputedNativeSource::tick(source)?;
        if index != 0 {
            skeleton.push(',');
        }
        skeleton.push('[');
        skeleton.push_str(&value.name);
        skeleton.push(']');
    }
    skeleton.push_str(" FROM [");
    skeleton.push_str(table);
    skeleton.push(']');

    source
        .budget
        .try_borrow_mut()
        .map_err(|_| QueryError::Source)?
        .charge(Resource::Steps, length as u64)
        .map_err(QueryError::from)?;
    let mut query = parse_query(&skeleton, limits)?.into_query();
    let Statement::Read(read) = &mut query.statement else {
        return Err(CalculationError::Unsupported);
    };
    let ReadBody::Select(select) = &mut read.body else {
        return Err(CalculationError::Unsupported);
    };
    for (projection, selected) in select.projection.iter_mut().zip(selected) {
        RecomputedNativeSource::tick(source)?;
        if let Some(expression) = selected.expression {
            // Keep the canonical field token as the result alias. The retained
            // formula never supplies SQL syntax or a name constructor.
            let original = std::mem::replace(
                projection,
                Projection::Expression {
                    expression,
                    alias: None,
                },
            );
            let Projection::Field { field, .. } = original else {
                return Err(CalculationError::Unsupported);
            };
            let Projection::Expression { alias, .. } = projection else {
                return Err(CalculationError::Unsupported);
            };
            *alias = Some(field.name);
        }
    }
    RecomputedNativeSource::tick(source)?;
    Ok(query)
}
