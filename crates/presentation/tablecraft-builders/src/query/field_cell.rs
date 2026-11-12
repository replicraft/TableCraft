//! Editable QBE field text retains aliases independently from source identity.
use super::{
    DesignColumn, DesignExpression, DesignField, DesignSource, DesignTableMetadata,
    QueryDesignError,
    build::{name, source_name},
};
use std::borrow::Cow;
use tablecraft_sql::{
    ParseErrorKind, ParseLimits, parse_expression_strict, parse_qbe_field_prefix,
    qbe_identifier_is_bare, select::Expression,
};

/// Render a grid-local field operand and its explicit QBE output alias.
///
/// Bound fields keep their separate Table-row identity. Ordinary local names
/// remain compact; identifiers whose spelling would be parsed as another
/// expression use the SQL owner's quoted-name renderer.
///
/// # Errors
/// Returns resource-limit errors. Invalid editable alias text stays visible;
/// normal Build still rejects it. No metadata is read or query executed.
pub fn field_cell_text(column: &DesignColumn) -> Result<Cow<'_, str>, QueryDesignError> {
    let operand = match &column.expression {
        DesignExpression::Field(field) => identifier_text(&field.column)?,
        DesignExpression::AllFields(_) => Cow::Borrowed("*"),
        DesignExpression::Expression(text) => Cow::Borrowed(text.as_str()),
    };
    match column.alias.as_deref() {
        Some(alias) => Ok(Cow::Owned(format!(
            "{}: {operand}",
            identifier_text(alias)?
        ))),
        None => Ok(operand),
    }
}

fn identifier_text(value: &str) -> Result<Cow<'_, str>, QueryDesignError> {
    let same_name = qbe_identifier_is_bare(value, ParseLimits::default()).map_err(|error| {
        if error.kind == ParseErrorKind::Limit {
            QueryDesignError::Limit
        } else {
            QueryDesignError::Parse(error)
        }
    })?;
    if same_name {
        Ok(Cow::Borrowed(value))
    } else {
        match name(value) {
            Ok(name) => Ok(Cow::Owned(name.to_string())),
            Err(QueryDesignError::Parse(error)) if error.kind == ParseErrorKind::Limit => {
                Err(QueryDesignError::Limit)
            }
            // This is visible draft text, not executable syntax. Preserve an
            // incomplete identifier until normal Build validates the draft.
            Err(QueryDesignError::Parse(_)) => Ok(Cow::Borrowed(value)),
            Err(error) => Err(error),
        }
    }
}

/// Decode a QBE alias prefix while borrowing the editable operand unchanged.
///
/// # Errors
/// Returns the SQL owner's bounded prefix lexical/identifier errors. The operand must
/// still pass normal source resolution and expression validation at Build.
pub fn parse_field_cell(text: &str) -> Result<(Option<String>, &str), QueryDesignError> {
    parse_qbe_field_prefix(text, ParseLimits::default()).map_err(|error| {
        if error.kind == ParseErrorKind::Limit {
            QueryDesignError::Limit
        } else {
            QueryDesignError::Parse(error)
        }
    })
}

/// Test field-cell syntax completeness without admitting its query semantics.
///
/// Incomplete aliases and expressions return `false` so editors can retain the
/// exact raw draft through focus changes. Source, role and callable admission
/// still belong to normal Build; a complete syntax tree grants no execution.
///
/// # Errors
/// Returns a resource-limit error without treating it as incomplete input.
pub fn field_cell_complete(text: &str) -> Result<bool, QueryDesignError> {
    let operand = match parse_field_cell(text) {
        Ok((_, operand)) => operand,
        Err(QueryDesignError::Limit) => return Err(QueryDesignError::Limit),
        Err(QueryDesignError::Parse(_)) => return Ok(false),
        Err(error) => return Err(error),
    };
    if operand.trim() == "*" {
        return Ok(true);
    }
    match parse_expression_strict(operand, ParseLimits::default()) {
        Ok(_) => Ok(true),
        Err(error) if error.kind == ParseErrorKind::Limit => Err(QueryDesignError::Limit),
        Err(_) => Ok(false),
    }
}

/// Edit a field operand against its known grid source or an explicit qualifier.
///
/// The source anchor preserves local names while typing. Quoted/qualified fields
/// use the SQL parser's real Field representation; unmatched text remains an
/// editable expression for normal Build validation rather than guessed metadata.
///
/// # Errors
/// Returns parser limits without reinterpreting them as field text. Missing or
/// ambiguous sources remain unbound expressions and must pass normal Build.
pub fn edit_field_operand(
    sources: &[DesignSource],
    metadata: &[DesignTableMetadata],
    source_id: Option<u64>,
    text: String,
) -> Result<DesignExpression, QueryDesignError> {
    if text == "*" {
        return Ok(DesignExpression::AllFields(source_id));
    }
    let parsed = match parse_expression_strict(&text, ParseLimits::default()) {
        Ok(Expression::Field(field)) => Some(field),
        Ok(_) => None,
        Err(error) if error.kind == ParseErrorKind::Limit => return Err(QueryDesignError::Limit),
        Err(_) => None,
    };
    let selected = match parsed.as_ref().and_then(|field| field.qualifier.as_ref()) {
        Some(qualifier) => {
            let mut matching = sources
                .iter()
                .filter(|source| source_name(source).eq_ignore_ascii_case(qualifier.as_str()));
            let first = matching.next();
            if matching.next().is_some() {
                None
            } else {
                first
            }
        }
        None => source_id.and_then(|id| sources.iter().find(|source| source.id == id)),
    };
    if let Some(source) = selected {
        let table = metadata
            .iter()
            .find(|table| table.table.eq_ignore_ascii_case(&source.table));
        let local = parsed
            .as_ref()
            .map_or(text.as_str(), |field| field.name.as_str());
        if let Some(field) = table.and_then(|table| {
            table
                .fields
                .iter()
                .find(|field| field.name.eq_ignore_ascii_case(local))
        }) {
            // NO_CLONE: Editable field spelling is an independent owned draft;
            // immutable metadata is borrowed and cannot move into that draft.
            let mut column = String::new();
            column
                .try_reserve_exact(field.name.len())
                .map_err(|_| QueryDesignError::Limit)?;
            column.push_str(&field.name);
            return Ok(DesignExpression::Field(DesignField {
                source_id: source.id,
                column,
            }));
        }
        if parsed
            .as_ref()
            .is_none_or(|field| field.qualifier.is_none())
            && table.is_some_and(|table| {
                table.fields.iter().any(|field| {
                    field
                        .name
                        .get(..text.len())
                        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(&text))
                })
            })
        {
            return Ok(DesignExpression::Field(DesignField {
                source_id: source.id,
                column: text,
            }));
        }
    }
    Ok(DesignExpression::Expression(text))
}

#[cfg(test)]
mod tests;
