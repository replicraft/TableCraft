//! The query owner resolves declarations; this boundary only accepts literal input.
use crate::{Error, display::Presentation, scalar_input};
use tablecraft_core::values::Value;
use tablecraft_query::{ParameterType, resolve_parameter_type};
use tablecraft_schema::{NameComparison, ScalarType};
use tablecraft_sql::query::Query;

/// Host input envelope; SQL/native declaration limits can be narrower.
pub const MAX_QUERY_PARAMETERS: usize = 256;

/// Declared prompt metadata. Inspection does not establish execution readiness.
pub struct ParameterSpec {
    pub name: String,
    /// Stable input token, or value when a concrete carrier must be selected.
    pub kind: String,
    /// Original declaration rendered by the syntax owner.
    pub label: String,
    pub nullable: bool,
}

/// Explicit literal input, never SQL or an expression to evaluate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterInput {
    pub name: String,
    pub kind: String,
    pub text: String,
    pub is_null: bool,
}

pub(crate) fn specs(
    query: &Query,
    output: &mut Presentation<'_>,
) -> Result<Vec<ParameterSpec>, Error> {
    if query.parameters.len() > MAX_QUERY_PARAMETERS {
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

pub(crate) fn bindings<'a>(
    query: &Query,
    inputs: &'a [ParameterInput],
    output: &mut Presentation<'_>,
) -> Result<Vec<(&'a str, Value)>, Error> {
    if inputs.len() > MAX_QUERY_PARAMETERS || inputs.len() != query.parameters.len() {
        return Err(Error::ParameterInput);
    }
    // Validate the complete declaration/input namespace before parsing any
    // values. Query binding uses this same ASCII-insensitive identifier policy;
    // the prompt protocol deliberately requires exact original input spelling.
    for (position, declaration) in query.parameters.iter().enumerate() {
        let name = declaration.name.as_str();
        for previous in &query.parameters[..position] {
            output.work(1 + name.len() as u64 + previous.name.as_str().len() as u64)?;
            if NameComparison::AsciiInsensitive.equals(name, previous.name.as_str()) {
                return Err(Error::ParameterInput);
            }
        }
        selected_input(name, inputs, output)?;
        output.work(1 + declaration.data_type.arguments.len() as u64)?;
        resolve_parameter_type(&declaration.data_type)?;
    }
    output.allocation(inputs.len(), size_of::<(&str, Value)>())?;
    let mut result = Vec::new();
    result.try_reserve_exact(inputs.len())?;
    for declaration in &query.parameters {
        let name = declaration.name.as_str();
        let input = selected_input(name, inputs, output)?;
        output.work(1 + input.kind.len() as u64 + declaration.data_type.arguments.len() as u64)?;
        let declared = resolve_parameter_type(&declaration.data_type)?;
        let value = parse_parameter_input(declared, input, output)?;
        // Exact matched spelling above lets the staged caller move its parsed
        // Query while these borrowed names remain owned by the supplied inputs.
        result.push((input.name.as_str(), value));
    }
    Ok(result)
}

fn selected_input<'a>(
    name: &str,
    inputs: &'a [ParameterInput],
    output: &mut Presentation<'_>,
) -> Result<&'a ParameterInput, Error> {
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
    selected.ok_or(Error::ParameterInput)
}

fn parse_parameter_input(
    declared: ParameterType,
    input: &ParameterInput,
    output: &mut Presentation<'_>,
) -> Result<Value, Error> {
    let kind = selected_kind(&input.kind).ok_or(Error::ParameterInput)?;
    if matches!(declared, ParameterType::Scalar(descriptor) if descriptor.scalar_type() != kind) {
        return Err(Error::ParameterInput);
    }
    let value = scalar_input::parse(kind, &input.text, input.is_null, output)?;
    if !declared.accepts(&value) {
        return Err(Error::ParameterInput);
    }
    Ok(value)
}

pub(crate) fn discovered_specs(
    prepared: &tablecraft_query::PreparedParameterQuery,
    original: &Query,
    output: &mut Presentation<'_>,
) -> Result<Vec<ParameterSpec>, Error> {
    let count = prepared.parameters().len();
    if count > MAX_QUERY_PARAMETERS {
        return Err(Error::ParameterInput);
    }
    output.allocation(count, size_of::<ParameterSpec>())?;
    let mut result = Vec::new();
    result.try_reserve_exact(count)?;
    for (index, parameter) in prepared.parameters().enumerate() {
        output.work(1)?;
        let kind = match parameter.data_type {
            ParameterType::Scalar(descriptor) => token(descriptor.scalar_type()),
            ParameterType::Value => "value",
        };
        let label = match parameter.origin {
            tablecraft_query::ParameterOrigin::Declared => {
                let declared = original
                    .parameters
                    .get(index)
                    .ok_or(Error::ParameterInput)?;
                if declared.name.as_str() != parameter.name
                    || resolve_parameter_type(&declared.data_type)? != parameter.data_type
                {
                    return Err(Error::ParameterInput);
                }
                output.text(&declared.data_type.to_string())?
            }
            tablecraft_query::ParameterOrigin::Implicit => output.text("VALUE")?,
        };
        result.push(ParameterSpec {
            name: output.text(parameter.name)?,
            kind: output.text(kind)?,
            label,
            nullable: true,
        });
    }
    Ok(result)
}

pub(crate) fn discovered_bindings<'a>(
    prepared: &'a tablecraft_query::PreparedParameterQuery,
    inputs: &[ParameterInput],
    output: &mut Presentation<'_>,
) -> Result<Vec<(&'a str, Value)>, Error> {
    let count = prepared.parameters().len();
    if inputs.len() > MAX_QUERY_PARAMETERS || inputs.len() != count {
        return Err(Error::ParameterInput);
    }
    output.allocation(count, size_of::<(&str, Value)>())?;
    let mut result = Vec::new();
    result.try_reserve_exact(count)?;
    for parameter in prepared.parameters() {
        let input = selected_input(parameter.name, inputs, output)?;
        output.work(1 + input.kind.len() as u64)?;
        let value = parse_parameter_input(parameter.data_type, input, output)?;
        result.push((parameter.name, value));
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

pub(crate) fn validate_input_limits(
    inputs: &[ParameterInput],
    output: &mut Presentation<'_>,
) -> Result<(), Error> {
    if inputs.len() > MAX_QUERY_PARAMETERS {
        return Err(Error::ScalarInputLimit);
    }
    let mut bytes = 0_usize;
    for input in inputs {
        output.work(1)?;
        for text in [&input.name, &input.kind, &input.text] {
            bytes = bytes
                .checked_add(text.len())
                .ok_or(Error::ScalarInputLimit)?;
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
        inputs[0].text = "雪".repeat(400_000);
        inputs[0].is_null = true;
        assert!(validate_input_limits(&inputs, &mut Presentation::new(&|| false)).is_ok());
        assert!(bindings(parsed.query(), &inputs, &mut Presentation::new(&|| false)).is_ok());
        assert!(matches!(
            validate_input_limits(&inputs[..1], &mut Presentation::new(&|| true)),
            Err(Error::Cancelled)
        ));
    }

    #[test]
    fn binding_names_borrow_supplied_inputs_and_the_complete_namespace_is_checked() {
        let query = parse_query(
            "PARAMETERS [Min] LONG; SELECT [Min]",
            ParseLimits::default(),
        )
        .unwrap()
        .into_query();
        let inputs = [ParameterInput {
            name: "Min".into(),
            kind: "int32".into(),
            text: "5".into(),
            is_null: false,
        }];
        let values = bindings(&query, &inputs, &mut Presentation::new(&|| false)).unwrap();
        drop(query);
        assert_eq!(values, [("Min", Value::Int32(5))]);
        assert_eq!(values[0].0.as_ptr(), inputs[0].name.as_ptr());

        for sql in [
            "PARAMETERS [Min] LONG, [Min] LONG; SELECT [Min]",
            "PARAMETERS [Min] LONG, [min] LONG; SELECT [Min]",
        ] {
            let query = parse_query(sql, ParseLimits::default()).unwrap();
            let inputs = [
                ParameterInput {
                    name: "Min".into(),
                    kind: "int32".into(),
                    text: "5".into(),
                    is_null: false,
                },
                ParameterInput {
                    name: "min".into(),
                    kind: "int32".into(),
                    text: "9".into(),
                    is_null: false,
                },
            ];
            assert!(matches!(
                bindings(query.query(), &inputs, &mut Presentation::new(&|| false)),
                Err(Error::ParameterInput)
            ));
        }
    }
}
