//! Independent complete fixed-key components and tuple NULL facts.
mod decimal;
mod floating;
mod guid;
mod numeric;

#[cfg(test)]
mod tests;

use super::{Wire, reserve};
use crate::{EditError, InspectError, NativeIndex, TableSchema, format::reader::Reader};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_schema::ScalarType;

const MAX_COMPONENTS: usize = 10;
const ROW_BYTES: usize = size_of::<u32>();
const MARKER_BYTES: usize = size_of::<u8>();
const ASC_NULL: u8 = 0;
const DESC_NULL: u8 = 0xff;

#[derive(Clone, Copy)]
enum Kind {
    Numeric(numeric::Profile),
    Decimal(decimal::Profile),
    Guid { descending: bool },
}

struct Component {
    column: usize,
    kind: Kind,
}

pub(super) struct Profile {
    components: Vec<Component>,
    pub required: bool,
    pub unique: bool,
    pub ignore_nulls: bool,
}

#[derive(Clone, Copy)]
pub(super) struct Nulls {
    pub all: bool,
    pub any: bool,
}

impl Profile {
    pub(super) fn bind(
        index: &NativeIndex,
        schema: &TableSchema,
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
            let kind = if scalar == ScalarType::Decimal {
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

    pub(super) fn request(
        &self,
        values: &[Value],
        row: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Wire, EditError> {
        if let [component] = self.components.as_slice() {
            let value = values
                .get(component.column)
                .ok_or(EditError::Preservation)?;
            let encoded = match component.kind {
                Kind::Numeric(profile) => profile.encode(profile.request(value)?, row)?,
                Kind::Decimal(profile) => profile.encode(value, row, reader)?,
                Kind::Guid { descending } => guid::encode(value, descending, row, reader)?,
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
            };
            let key = &encoded[..encoded.len() - ROW_BYTES];
            reserve(&mut bytes, key.len(), reader)?;
            reader.work(key.len())?;
            bytes.extend_from_slice(key);
        }
        reserve(&mut bytes, ROW_BYTES, reader)?;
        bytes.extend_from_slice(&row.to_be_bytes());
        let wire = Wire::read_paid(&bytes, reader)?;
        self.value(&wire)?;
        Ok(wire)
    }

    pub(super) fn value(&self, wire: &Wire) -> Result<Nulls, EditError> {
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
