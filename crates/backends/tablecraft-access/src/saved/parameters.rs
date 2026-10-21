use super::{SavedSelectError, context::Context, identifier};
use tablecraft_ace::{QueryAttribute, SavedQueryDefinition};
use tablecraft_core::limits::Resource;
use tablecraft_sql::query::{DataType, Parameter};

// Match the existing owned SQL parser's declaration ceiling, not a claim about
// every native engine/provider's parameter limit.
pub(super) const MAX_DECLARATIONS: usize = 255;
const BOOLEAN: i16 = 1;
const BYTE: i16 = 2;
const SHORT: i16 = 3;
const LONG: i16 = 4;
const CURRENCY: i16 = 5;
const SINGLE: i16 = 6;
const DOUBLE: i16 = 7;
const DATE_TIME: i16 = 8;
const BINARY: i16 = 9;
const TEXT: i16 = 10;
const LONG_BINARY: i16 = 11;
const GUID: i16 = 15;

pub(super) fn read(
    definition: &SavedQueryDefinition,
    count: usize,
    context: &mut Context<'_>,
) -> Result<Vec<Parameter>, SavedSelectError> {
    let mut result: Vec<Parameter> = context.reserve(count)?;
    for row in definition.rows() {
        context.check()?;
        if row.attribute() != Some(QueryAttribute::Parameter) {
            continue;
        }
        let name = row.name1().ok_or(SavedSelectError::InvalidMetadata)?;
        let name = identifier::literal(name)?;
        // Literal ASCII prompt/name matching, without approximating native
        // Unicode identity or qualified Forms!/dot name syntax.
        if !name.is_ascii() || name.contains('!') {
            return Err(SavedSelectError::Unsupported);
        }
        for before in &result {
            context.check()?;
            context.budget.charge(
                Resource::Steps,
                (name.len() + before.name.as_str().len()) as u64,
            )?;
            if before.name.as_str().eq_ignore_ascii_case(name) {
                return Err(SavedSelectError::InvalidMetadata);
            }
        }
        let flag = row.flag().ok_or(SavedSelectError::InvalidMetadata)?;
        let type_name = match flag {
            BOOLEAN => "BIT",
            BYTE => "BYTE",
            SHORT => "SHORT",
            LONG => "LONG",
            CURRENCY => "CURRENCY",
            SINGLE => "IEEESINGLE",
            DOUBLE => "IEEEDOUBLE",
            DATE_TIME => "DATETIME",
            BINARY => "BINARY",
            TEXT => "TEXT",
            LONG_BINARY => "LONGBINARY",
            GUID => "GUID",
            _ => return Err(SavedSelectError::Unsupported),
        };
        let length = match (flag, row.extra()) {
            (TEXT, None | Some(0)) => None,
            (TEXT, Some(length)) if length > 0 => Some(length as u32),
            (_, None) => None,
            _ => return Err(SavedSelectError::Unsupported),
        };
        let name = context.name(name)?;
        let name_type = context.text(&[type_name])?;
        let mut arguments = context.reserve(usize::from(length.is_some()))?;
        if let Some(length) = length {
            context.syntax(length.ilog10() as usize + "0()".len())?;
            arguments.push(length);
        }
        result.push(Parameter {
            name,
            data_type: DataType {
                name: name_type,
                arguments,
            },
        });
    }
    context.check()?;
    Ok(result)
}
