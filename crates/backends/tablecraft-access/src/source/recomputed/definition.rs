//! Checked owned query construction; stored formula text is never interpolated.
use super::{CalculationView, MAX_NAME_BYTES, MAX_VIEW_COLUMNS, RecomputedNativeSource, View};
use crate::source::{CalculationError, NativeSource};
use tablecraft_ace::{NativeReadSource, NativeTableStorage};
use tablecraft_core::limits::Resource;
use tablecraft_query::QueryError;
use tablecraft_sql::{
    ParseError, ParseErrorKind, ParseLimits, SourceLocation,
    query::{Query, ReadBody, ReadQuery, Statement},
    select::{Expression, FieldReference, Name, Projection, Select, Table, TableSource},
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
    // Retain the former carrier allowance for checked names and owned formulas.
    // Identifiers and expressions now enter the owned model directly.
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
    // Retained identifier copies (including formula aliases) and the FROM model
    // keep the existing conservative allocation allowance.
    charge(source, length, size_of::<char>())?;

    source
        .budget
        .try_borrow_mut()
        .map_err(|_| QueryError::Source)?
        .charge(Resource::Steps, length as u64)
        .map_err(QueryError::from)?;

    // Keep the former SELECT carrier's input/token ceilings without reparsing
    // identifiers or already-validated formula trees.
    let tokens = selected
        .len()
        .checked_mul(2)
        .and_then(|count| count.checked_add(2))
        .ok_or(QueryError::StructureLimit)?;
    // The old reservation included one unused trailing comma byte.
    let syntax_bytes = length.checked_sub(1).ok_or(QueryError::StructureLimit)?;
    let message = if syntax_bytes > limits.max_bytes.min(ParseLimits::MAX_BYTES) {
        Some("SQL byte limit exceeded")
    } else if tokens > limits.max_tokens.min(ParseLimits::MAX_TOKENS) {
        Some("SQL token limit exceeded")
    } else if limits.max_depth == 0 {
        Some("SQL expression depth limit exceeded")
    } else {
        None
    };
    if let Some(message) = message {
        return Err(ParseError {
            kind: ParseErrorKind::Limit,
            message,
            location: SourceLocation { line: 1, column: 1 },
        }
        .into());
    }

    let mut projections = Vec::new();
    projections
        .try_reserve_exact(selected.len())
        .map_err(|_| QueryError::Allocation)?;
    for selected in selected {
        RecomputedNativeSource::tick(source)?;
        let name = Name::from_identifier(&selected.name, limits)?;
        let projection = match selected.expression {
            Some(expression) => Projection::Expression {
                expression,
                alias: Some(name),
            },
            None => Projection::Field {
                field: FieldReference {
                    qualifier: None,
                    name,
                },
                alias: None,
            },
        };
        projections.push(projection);
    }

    let query = Query::from_select(Select::from_projection(
        projections,
        vec![TableSource::Table(Table {
            database: None,
            name: Name::from_identifier(table, limits)?,
            alias: None,
        })],
    ));

    RecomputedNativeSource::tick(source)?;
    Ok(query)
}
