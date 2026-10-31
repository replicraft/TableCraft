//! Explicit creation metadata, validated before opening the optional data file.
use super::{Error, input::reserve, tick};
use tablecraft_access::creation::CreationError;
use tablecraft_ace::{CreateColumn, InspectError};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::{DecimalConstraints, ScalarType};

const FIELD_WORDS: usize = 2;
const MAX_COLUMNS: usize = 16;

pub(super) struct Field<'a> {
    pub column: CreateColumn<'a>,
    pub kind: &'static str,
}

pub(super) fn parse<'a>(
    words: &'a [String],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Field<'a>>, Error> {
    if words.is_empty()
        || !words.len().is_multiple_of(FIELD_WORDS)
        || words.len() / FIELD_WORDS > MAX_COLUMNS
    {
        return Err(Error::Input(
            "Typed creation requires one through sixteen complete fields",
        ));
    }
    let mut fields = Vec::new();
    reserve(&mut fields, words.len() / FIELD_WORDS, budget)?;
    for pair in words.as_chunks::<FIELD_WORDS>().0 {
        tick(budget, cancelled)?;
        budget
            .charge(Resource::Steps, pair[1].len() as u64)
            .map_err(InspectError::from)?;
        let (column, kind) = declaration(&pair[0], &pair[1])?;
        fields.push(Field { column, kind });
    }
    Ok(fields)
}

fn declaration<'a>(
    name: &'a str,
    declaration: &str,
) -> Result<(CreateColumn<'a>, &'static str), Error> {
    let fixed = match declaration {
        "boolean" => Some((ScalarType::Boolean, "boolean")),
        "byte" => Some((ScalarType::Byte, "byte")),
        "int16" => Some((ScalarType::Int16, "int16")),
        "int32" => Some((ScalarType::Int32, "int32")),
        "int64" => Some((ScalarType::Int64, "int64")),
        "currency-scaled" => Some((ScalarType::Currency, "currency-scaled")),
        "single-bits" => Some((ScalarType::Single, "single-bits")),
        "double-bits" => Some((ScalarType::Double, "double-bits")),
        "date-bits" => Some((ScalarType::DateTime, "date-bits")),
        "guid" => Some((ScalarType::Guid, "guid")),
        _ => None,
    };
    if let Some((scalar, kind)) = fixed {
        return Ok((CreateColumn::new(name, scalar), kind));
    }
    if let Some(units) = declaration.strip_prefix("text:") {
        return Ok((
            CreateColumn::short_text(name, dimension(units)?)
                .map_err(|cause| Error::Creation(CreationError::Create(cause)))?,
            "text",
        ));
    }
    if let Some(bytes) = declaration.strip_prefix("binary:") {
        return Ok((
            CreateColumn::short_binary(name, dimension(bytes)?)
                .map_err(|cause| Error::Creation(CreationError::Create(cause)))?,
            "binary",
        ));
    }
    if let Some(parameters) = declaration.strip_prefix("decimal:") {
        let (precision, scale) = parameters
            .split_once(':')
            .ok_or(Error::Input("Expected decimal:PRECISION:SCALE"))?;
        let precision = u8::try_from(dimension(precision)?)
            .map_err(|_| Error::Input("Invalid Decimal precision"))?;
        let scale =
            u8::try_from(dimension(scale)?).map_err(|_| Error::Input("Invalid Decimal scale"))?;
        let constraints = DecimalConstraints::new(precision, scale)
            .map_err(|_| Error::Input("Invalid Decimal precision or scale"))?;
        return Ok((CreateColumn::decimal(name, constraints), "decimal"));
    }
    Err(Error::Input(
        "Unknown creation declaration or missing native parameters",
    ))
}

fn dimension(text: &str) -> Result<usize, Error> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Error::Input(
            "Expected unsigned decimal native declaration dimensions",
        ));
    }
    text.parse()
        .map_err(|_| Error::Input("Native declaration dimension is outside its range"))
}
