//! Owned PARAMETERS syntax round-trips through the existing strict SQL owner.
use super::{MAX_DESIGN_TEXT_BYTES, QueryDesignError};
use std::fmt::Write;
use tablecraft_sql::{
    ParseLimits, parse_query,
    query::{Parameter, Query},
    select::{Expression, Projection, Select},
};

fn statement() -> Query {
    Query::from_select(Select::from_projection(
        vec![Projection::Expression {
            expression: Expression::Integer(1),
            alias: None,
        }],
        Vec::new(),
    ))
}

/// Parse a declaration header without executing, prompting or accepting a query.
///
/// # Errors
/// Rejects invalid or duplicate declarations, extra statements and editor limits.
pub fn parse_parameters(text: &str) -> Result<Vec<Parameter>, QueryDesignError> {
    if text.len() > MAX_DESIGN_TEXT_BYTES {
        return Err(QueryDesignError::Limit);
    }
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let sql = format!("{text}\nSELECT 1;");
    let parsed = parse_query(&sql, ParseLimits::default())?.into_query();
    if parsed.statement != statement().statement || parsed.parameters.is_empty() {
        return Err(QueryDesignError::Unsupported);
    }
    for (position, parameter) in parsed.parameters.iter().enumerate() {
        if parsed.parameters[..position].iter().any(|other| {
            other
                .name
                .as_str()
                .eq_ignore_ascii_case(parameter.name.as_str())
        }) {
            return Err(QueryDesignError::InvalidReference);
        }
    }
    Ok(parsed.parameters)
}

/// Render the canonical declaration header, using the actual Name/Type owners.
///
/// # Errors
/// Rejects malformed owner values, duplicate declarations or editor limits.
pub fn render_parameters(parameters: &[Parameter]) -> Result<String, QueryDesignError> {
    if parameters.is_empty() {
        return Ok(String::new());
    }
    let mut text = String::from("PARAMETERS ");
    for (position, parameter) in parameters.iter().enumerate() {
        if parameter.name.as_str().len() > MAX_DESIGN_TEXT_BYTES
            || parameter.data_type.name.len() > MAX_DESIGN_TEXT_BYTES
            || parameter.data_type.arguments.len() > MAX_DESIGN_TEXT_BYTES
        {
            return Err(QueryDesignError::Limit);
        }
        if position != 0 {
            text.push_str(", ");
        }
        write!(&mut text, "{} {}", parameter.name, parameter.data_type)
            .map_err(|_| QueryDesignError::Limit)?;
        if text.len() > MAX_DESIGN_TEXT_BYTES {
            return Err(QueryDesignError::Limit);
        }
    }
    text.push(';');
    if parse_parameters(&text)?.as_slice() != parameters {
        return Err(QueryDesignError::Unsupported);
    }
    Ok(text)
}
