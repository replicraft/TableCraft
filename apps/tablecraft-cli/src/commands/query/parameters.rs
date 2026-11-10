use super::Error;
use tablecraft_core::{limits::ResourceBudget, values::Value};

mod currency;
mod numeric;

const MAX_BINDINGS: usize = 256;
const MAX_NAME_BYTES: usize = 255;

pub(super) fn parse(raw: &[String]) -> Result<Vec<(&str, Value)>, Error> {
    if raw.len() > MAX_BINDINGS {
        return Err(Error::Parameter);
    }
    let mut result: Vec<(&str, Value)> = Vec::new();
    result
        .try_reserve_exact(raw.len())
        .map_err(|_| Error::Parameter)?;
    let mut conversion_budget = ResourceBudget::new(u64::MAX, 0, u64::MAX);
    for raw in raw {
        let (name, literal) = assignment(raw)?;
        if name.is_empty()
            || name.len() > MAX_NAME_BYTES
            || name.chars().any(char::is_control)
            || result.iter().any(|(n, _)| n.eq_ignore_ascii_case(name))
        {
            return Err(Error::Parameter);
        }
        let (kind, text) = literal.split_once(':').ok_or(Error::Parameter)?;
        let value = match kind {
            "null" if text.is_empty() => Value::Null,
            "boolean" => Value::Boolean(match text {
                "true" => true,
                "false" => false,
                _ => return Err(Error::Parameter),
            }),
            "byte" => Value::Byte(text.parse().map_err(|_| Error::Parameter)?),
            "short" => Value::Int16(text.parse().map_err(|_| Error::Parameter)?),
            "long" => Value::Int32(text.parse().map_err(|_| Error::Parameter)?),
            "bigint" => Value::Int64(text.parse().map_err(|_| Error::Parameter)?),
            "currency" => currency::parse(text, &mut conversion_budget)?,
            "decimal" => numeric::decimal(text)?,
            "single" => numeric::single(text)?,
            "double" => numeric::double(text)?,
            "date" => Value::DateTime(
                tablecraft_expressions::datetime::parse_literal(text)
                    .map_err(|_| Error::Parameter)?,
            ),
            "guid" => Value::Guid(
                tablecraft_expressions::guid::parse_literal(text).map_err(|_| Error::Parameter)?,
            ),
            "text" => {
                let mut value = String::new();
                value
                    .try_reserve_exact(text.len())
                    .map_err(|_| Error::Parameter)?;
                value.push_str(text);
                Value::Text(value)
            }
            _ => return Err(Error::Parameter),
        };
        result.push((name, value));
    }
    Ok(result)
}

fn assignment(raw: &str) -> Result<(&str, &str), Error> {
    if let Some(body) = raw.strip_prefix('[') {
        let (name, rest) = body.split_once(']').ok_or(Error::Parameter)?;
        if name.contains('[') {
            return Err(Error::Parameter);
        }
        return Ok((name, rest.strip_prefix('=').ok_or(Error::Parameter)?));
    }
    raw.split_once('=').ok_or(Error::Parameter)
}

#[cfg(test)]
mod tests;
