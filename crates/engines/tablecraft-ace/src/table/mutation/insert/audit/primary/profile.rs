//! Independent complete fixed-key components and tuple NULL facts.
mod decimal;
mod digest;
mod floating;
mod guid;
mod numeric;
mod variable;

#[cfg(test)]
mod tests;

use super::{Wire, reserve};
use crate::{
    EditError, InspectError, NativeColumn, NativeIndex, TableSchema,
    format::{
        data_type::{SHORT_BINARY, SHORT_TEXT},
        reader::Reader,
    },
};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_schema::ScalarType;

const MAX_COMPONENTS: usize = 10;
const ROW_BYTES: usize = size_of::<u32>();
const MARKER_BYTES: usize = size_of::<u8>();
const ASC_NULL: u8 = 0;
const DESC_NULL: u8 = 0xff;

#[derive(Clone, Copy)]
enum Kind<'a> {
    Numeric(numeric::Profile),
    Decimal(decimal::Profile),
    Text {
        field: &'a NativeColumn,
        descending: bool,
    },
    Binary {
        maximum: usize,
        descending: bool,
    },
    Guid {
        descending: bool,
    },
}

struct Component<'a> {
    column: usize,
    kind: Kind<'a>,
}

pub(super) struct Profile<'a> {
    components: Vec<Component<'a>>,
    pub required: bool,
    pub unique: bool,
    pub ignore_nulls: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Nulls {
    pub all: bool,
    pub any: bool,
}

impl<'a> Profile<'a> {
    pub(super) fn bind(
        index: &NativeIndex,
        schema: &'a TableSchema,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Self, EditError> {
        if !(1..=MAX_COMPONENTS).contains(&index.columns().len()) {
            return Err(EditError::Preservation);
        }
        let constraints = index.constraints().ok_or(EditError::Preservation)?;
        if constraints.ignore_nulls() && constraints.required() {
            return Err(EditError::Preservation);
        }
        let mut components = Vec::new();
        reserve(&mut components, index.columns().len(), reader)?;
        for key in index.columns() {
            let mut selected = None;
            for (position, field) in schema.columns().iter().enumerate() {
                reader.tick()?;
                if field.number() == key.number() && selected.replace(position).is_some() {
                    return Err(EditError::Preservation);
                }
            }
            let column = selected.ok_or(EditError::Preservation)?;
            let scalar = schema.columns()[column]
                .scalar_type()
                .ok_or(EditError::Preservation)?;
            let field = &schema.columns()[column];
            let kind = if scalar == ScalarType::Text && field.native_type() == SHORT_TEXT {
                let order = field.text_sort_order().ok_or(EditError::Preservation)?;
                let general = order.general_family().is_some();
                if !general && (index.columns().len() != 1 || !order.is_french_legacy()) {
                    return Err(EditError::Preservation);
                }
                if field.storage_length() == 0
                    || field.storage_length()
                        > variable::MAX_SHORT_TEXT_UNITS * variable::UTF16_BYTES
                    || !field.storage_length().is_multiple_of(variable::UTF16_BYTES)
                {
                    return Err(EditError::Preservation);
                }
                Kind::Text {
                    field,
                    descending: key.descending(),
                }
            } else if scalar == ScalarType::Binary && field.native_type() == SHORT_BINARY {
                if !(1..=variable::MAX_SHORT_BINARY).contains(&field.storage_length()) {
                    return Err(EditError::Preservation);
                }
                Kind::Binary {
                    maximum: field.storage_length(),
                    descending: key.descending(),
                }
            } else if scalar == ScalarType::Decimal {
                let (precision, scale) = schema.columns()[column]
                    .decimal_parameters()
                    .ok_or(EditError::Preservation)?;
                Kind::Decimal(decimal::Profile::new(precision, scale, key.descending())?)
            } else if scalar == ScalarType::Guid {
                Kind::Guid {
                    descending: key.descending(),
                }
            } else {
                let width = match scalar {
                    ScalarType::Boolean | ScalarType::Byte => size_of::<u8>(),
                    ScalarType::Int16 => size_of::<i16>(),
                    ScalarType::Int32 | ScalarType::Single => size_of::<i32>(),
                    ScalarType::Int64
                    | ScalarType::Currency
                    | ScalarType::Double
                    | ScalarType::DateTime => size_of::<i64>(),
                    _ => return Err(EditError::Preservation),
                };
                let numeric = numeric::Profile {
                    kind: scalar,
                    width,
                    descending: key.descending(),
                    required: false,
                };
                Kind::Numeric(numeric)
            };
            components.push(Component { column, kind });
        }
        Ok(Self {
            components,
            required: constraints.required(),
            unique: constraints.unique(),
            ignore_nulls: constraints.ignore_nulls(),
        })
    }

    pub(super) fn columns(
        &self,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Vec<usize>, EditError> {
        let mut columns = Vec::new();
        reserve(&mut columns, self.components.len(), reader)?;
        for component in &self.components {
            reader.tick()?;
            columns.push(component.column);
        }
        Ok(columns)
    }

    pub(super) fn variable(&self) -> bool {
        self.components
            .iter()
            .any(|component| matches!(component.kind, Kind::Text { .. } | Kind::Binary { .. }))
    }

    fn digested(&self) -> bool {
        self.components
            .iter()
            .any(|component| matches!(component.kind, Kind::Text { .. }))
            || (self.components.len() > 1 && self.variable())
    }

    pub(super) fn exact_unique(&self, wire: &Wire) -> bool {
        !self.digested() || wire.len() - ROW_BYTES < digest::MAX_KEY_BYTES
    }

    pub(super) fn row_nulls(
        &self,
        values: &[Value],
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Nulls, EditError> {
        let mut result = Nulls {
            all: true,
            any: false,
        };
        for component in &self.components {
            reader.tick()?;
            let value = values
                .get(component.column)
                .ok_or(EditError::Preservation)?;
            let null = matches!(value, Value::Null);
            result.all &= null;
            result.any |= null;
        }
        if self.required && result.any {
            return Err(EditError::Preservation);
        }
        Ok(result)
    }

    pub(super) fn request(
        &self,
        values: &[Value],
        row: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Wire, EditError> {
        self.row_nulls(values, reader)?;
        if let [component] = self.components.as_slice()
            && !self.variable()
        {
            let value = values
                .get(component.column)
                .ok_or(EditError::Preservation)?;
            let encoded = match component.kind {
                Kind::Numeric(profile) => profile.encode(profile.request(value)?, row)?,
                Kind::Decimal(profile) => profile.encode(value, row, reader)?,
                Kind::Guid { descending } => guid::encode(value, descending, row, reader)?,
                Kind::Text { .. } | Kind::Binary { .. } => return Err(EditError::Preservation),
            };
            self.value(&encoded)?;
            return Ok(encoded);
        }

        let mut bytes = Vec::new();
        for component in &self.components {
            reader.tick()?;
            let value = values
                .get(component.column)
                .ok_or(EditError::Preservation)?;
            let encoded = match component.kind {
                Kind::Numeric(profile) => {
                    let atom = profile.request(value)?;
                    profile.encode(atom, 0)?
                }
                Kind::Decimal(profile) => profile.encode(value, 0, reader)?,
                Kind::Guid { descending } => guid::encode(value, descending, 0, reader)?,
                Kind::Text { field, descending } => {
                    variable::text(field, descending, value, &mut bytes, reader)?;
                    continue;
                }
                Kind::Binary {
                    maximum,
                    descending,
                } => {
                    variable::binary(maximum, descending, value, &mut bytes, reader)?;
                    continue;
                }
            };
            let key = &encoded[..encoded.len() - ROW_BYTES];
            reserve(&mut bytes, key.len(), reader)?;
            reader.work(key.len())?;
            bytes.extend_from_slice(key);
        }
        if self.digested() {
            digest::truncate(&mut bytes, reader)?;
        }
        reserve(&mut bytes, ROW_BYTES, reader)?;
        bytes.extend_from_slice(&row.to_be_bytes());
        let wire = Wire::read_paid(&bytes, reader)?;
        if !self.variable() {
            self.value(&wire)?;
        }
        Ok(wire)
    }

    pub(super) fn value(&self, wire: &Wire) -> Result<Nulls, EditError> {
        if self.variable() {
            // Complete row facts are required: a truncated physical tuple cannot
            // reveal component NULLs or certify a unique semantic value.
            return Err(EditError::Preservation);
        }
        let end = wire
            .len()
            .checked_sub(ROW_BYTES)
            .ok_or(EditError::Preservation)?;
        let mut offset = 0;
        let mut nulls = Nulls {
            all: true,
            any: false,
        };
        for component in &self.components {
            let bytes = wire.get(offset..end).ok_or(EditError::Preservation)?;
            let (null, length) = match component.kind {
                Kind::Numeric(profile) => {
                    let null_marker = if profile.descending {
                        DESC_NULL
                    } else {
                        ASC_NULL
                    };
                    let length = if bytes.first() == Some(&null_marker) {
                        MARKER_BYTES
                    } else {
                        MARKER_BYTES + profile.width
                    };
                    let mut encoded = [0; 1 + size_of::<i64>() + ROW_BYTES];
                    encoded[..length]
                        .copy_from_slice(bytes.get(..length).ok_or(EditError::Preservation)?);
                    let leaf = Wire::read(&encoded[..length + ROW_BYTES])?;
                    (profile.value(&leaf)?.is_none(), length)
                }
                Kind::Decimal(profile) => profile.validate(bytes)?,
                Kind::Guid { descending } => guid::validate(bytes, descending)?,
                Kind::Text { .. } | Kind::Binary { .. } => return Err(EditError::Preservation),
            };
            nulls.all &= null;
            nulls.any |= null;
            offset = offset
                .checked_add(length)
                .ok_or(InspectError::StructureLimit)?;
        }
        if offset != end || (self.required && nulls.any) {
            return Err(EditError::Preservation);
        }
        Ok(nulls)
    }
}
