use super::Error;
use tablecraft_core::values::Value;

const MAX_BINDINGS: usize = 256;
const MAX_NAME_BYTES: usize = 255;
const MAX_ARGUMENT_BYTES: usize = 65_536;

pub(super) fn parse(raw: &[String]) -> Result<Vec<(String, Value)>, Error> {
    if raw.len() > MAX_BINDINGS || raw.iter().map(String::len).sum::<usize>() > MAX_ARGUMENT_BYTES {
        return Err(Error::Parameter);
    }
    let mut result: Vec<(String, Value)> = Vec::new();
    for raw in raw {
        let (name, literal) = raw.split_once('=').ok_or(Error::Parameter)?;
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
            "text" => Value::Text(text.into()),
            _ => return Err(Error::Parameter),
        };
        result.push((name.into(), value));
    }
    Ok(result)
}
