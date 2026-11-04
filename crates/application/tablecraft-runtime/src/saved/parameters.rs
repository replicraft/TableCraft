//! The query owner resolves declarations; this boundary only accepts literal input.
use super::{
    MAX_SAVED_QUERY_INPUT_BYTES, MAX_SAVED_QUERY_PARAMETERS, ParameterInput, ParameterSpec,
};
use crate::{Error, display::Presentation, scalar_input};
use tablecraft_core::values::Value;
use tablecraft_query::{ParameterType, resolve_parameter_type};
use tablecraft_schema::ScalarType;
use tablecraft_sql::query::Query;

pub(super) fn specs(
    query: &Query,
    output: &mut Presentation<'_>,
) -> Result<Vec<ParameterSpec>, Error> {
    if query.parameters.len() > MAX_SAVED_QUERY_PARAMETERS {
        return Err(Error::ParameterInput);
    }
    output.allocation(query.parameters.len(), size_of::<ParameterSpec>())?;
    let mut result = Vec::new();
    result.try_reserve_exact(query.parameters.len())?;
    for declaration in &query.parameters {
        output.work(1 + declaration.data_type.arguments.len() as u64)?;
        let kind = match resolve_parameter_type(&declaration.data_type)? {
            ParameterType::Scalar(descriptor) => token(descriptor.scalar_type()),
            ParameterType::Value => "value",
        };
        result.push(ParameterSpec {
            name: output.text(declaration.name.as_str())?,
            kind: output.text(kind)?,
            label: output.text(&declaration.data_type.to_string())?,
            nullable: true,
        });
    }
    Ok(result)
}

pub(super) fn bindings<'a>(
    query: &'a Query,
    inputs: &[ParameterInput],
    output: &mut Presentation<'_>,
) -> Result<Vec<(&'a str, Value)>, Error> {
    if inputs.len() > MAX_SAVED_QUERY_PARAMETERS || inputs.len() != query.parameters.len() {
        return Err(Error::ParameterInput);
    }
    output.allocation(inputs.len(), size_of::<(&str, Value)>())?;
    let mut result = Vec::new();
    result.try_reserve_exact(inputs.len())?;
    for declaration in &query.parameters {
        let name = declaration.name.as_str();
        let mut selected = None;
        for input in inputs {
            output.work(1 + name.len() as u64 + input.name.len() as u64)?;
            if input.name == name {
                if selected.is_some() {
                    return Err(Error::ParameterInput);
                }
                selected = Some(input);
            }
        }
        let input = selected.ok_or(Error::ParameterInput)?;
        output.work(1 + input.kind.len() as u64 + declaration.data_type.arguments.len() as u64)?;
        let declared = resolve_parameter_type(&declaration.data_type)?;
        let kind = selected_kind(&input.kind).ok_or(Error::ParameterInput)?;
        if matches!(declared, ParameterType::Scalar(descriptor) if descriptor.scalar_type() != kind)
        {
            return Err(Error::ParameterInput);
        }
        let value = scalar_input::parse(kind, &input.text, input.is_null, output)?;
        if !declared.accepts(&value) {
            return Err(Error::ParameterInput);
        }
        result.push((name, value));
    }
    Ok(result)
}

fn token(kind: ScalarType) -> &'static str {
    match kind {
        ScalarType::Boolean => "boolean",
        ScalarType::Byte => "byte",
        ScalarType::Int16 => "int16",
        ScalarType::Int32 => "int32",
        ScalarType::Int64 => "int64",
        ScalarType::Single => "single",
        ScalarType::Double => "double",
        ScalarType::Currency => "currency",
        ScalarType::Decimal => "decimal",
        ScalarType::Text => "text",
        ScalarType::Binary => "binary",
        ScalarType::Guid => "guid",
        ScalarType::DateTime => "date",
        ScalarType::DateTimeExtended => "datetime_extended",
    }
}

fn selected_kind(token: &str) -> Option<ScalarType> {
    Some(match token {
        "boolean" => ScalarType::Boolean,
        "byte" => ScalarType::Byte,
        "int16" => ScalarType::Int16,
        "int32" => ScalarType::Int32,
        "int64" => ScalarType::Int64,
        "single" => ScalarType::Single,
        "double" => ScalarType::Double,
        "currency" => ScalarType::Currency,
        "decimal" => ScalarType::Decimal,
        "text" => ScalarType::Text,
        "binary" => ScalarType::Binary,
        "guid" => ScalarType::Guid,
        "date" => ScalarType::DateTime,
        "datetime_extended" => ScalarType::DateTimeExtended,
        _ => return None,
    })
}

pub(super) fn validate_input_limits(
    inputs: &[ParameterInput],
    output: &mut Presentation<'_>,
) -> Result<(), Error> {
    if inputs.len() > MAX_SAVED_QUERY_PARAMETERS {
        return Err(Error::ScalarInputLimit);
    }
    let mut bytes = 0_usize;
    for input in inputs {
        output.work(1)?;
        for text in [&input.name, &input.kind, &input.text] {
            bytes = bytes
                .checked_add(text.len())
                .ok_or(Error::ScalarInputLimit)?;
            if bytes > MAX_SAVED_QUERY_INPUT_BYTES {
                return Err(Error::ScalarInputLimit);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tablecraft_sql::{ParseLimits, parse_query};

    #[test]
    fn declaration_constraints_and_input_limits_remain_at_their_actual_boundaries() {
        let parsed = parse_query(
            "PARAMETERS [Text] TEXT(2), [Amount] DECIMAL(5,2); SELECT [Text]",
            ParseLimits::default(),
        )
        .unwrap();
        let mut inputs = vec![
            ParameterInput {
                name: "Text".into(),
                kind: "text".into(),
                text: "abc".into(),
                is_null: false,
            },
            ParameterInput {
                name: "Amount".into(),
                kind: "decimal".into(),
                text: "1.23".into(),
                is_null: false,
            },
        ];
        assert!(matches!(
            bindings(parsed.query(), &inputs, &mut Presentation::new(&|| false)),
            Err(Error::ParameterInput)
        ));
        inputs[0].text = "ab".into();
        assert!(bindings(parsed.query(), &inputs, &mut Presentation::new(&|| false)).is_ok());
        inputs[0].text = "x".repeat(MAX_SAVED_QUERY_INPUT_BYTES);
        inputs[0].is_null = true;
        assert!(matches!(
            validate_input_limits(&inputs, &mut Presentation::new(&|| false)),
            Err(Error::ScalarInputLimit)
        ));
        assert!(matches!(
            validate_input_limits(&inputs[..1], &mut Presentation::new(&|| true)),
            Err(Error::Cancelled)
        ));
    }
}
