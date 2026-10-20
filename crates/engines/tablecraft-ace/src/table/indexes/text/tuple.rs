//! Owned physical tuples containing text. Digest the complete tuple only once.
use super::super::{
    NativeIndex, binary,
    decimal::DecimalType,
    guid,
    key::{ASC_NULL, ASC_VALUE, DESC_NULL, DESC_VALUE, KeyProfile, KeyRow, KeyWork},
    numeric::{NumericKey, encode_component},
    profile::{KeyType, qualify},
    text::{self, TextProfile},
};
use crate::{
    InspectError, NativeColumn, TableSchema,
    format::{data_type, layout::INDEX_KEY_COLUMNS},
    header::check,
};
use std::cmp::Ordering;
use tablecraft_core::{limits::Resource, values::Value};
use tablecraft_schema::ScalarType;

const SCALAR_BUFFER_BYTES: usize = binary::MAX_ENTRY_BYTES;

enum Kind<'a> {
    Text(&'a NativeColumn),
    Numeric(KeyType, NumericKey),
    Guid,
    Binary(usize),
    Decimal(DecimalType),
}
struct Component<'a> {
    column: usize,
    descending: bool,
    kind: Kind<'a>,
}
impl<'a> Component<'a> {
    fn new(column: usize, field: &'a NativeColumn, descending: bool) -> Result<Self, InspectError> {
        let kind = match field.scalar_type().ok_or(InspectError::Unsupported)? {
            ScalarType::Text
                if matches!(
                    field.native_type(),
                    data_type::SHORT_TEXT | data_type::LONG_TEXT
                ) =>
            {
                TextProfile::new(column, field, descending)?;
                Kind::Text(field)
            }
            ScalarType::Guid => Kind::Guid,
            ScalarType::Binary if field.native_type() == data_type::SHORT_BINARY => {
                Kind::Binary(field.storage_length())
            }
            ScalarType::Decimal => Kind::Decimal(DecimalType::from_column(field)?),
            kind => Kind::Numeric(
                KeyType::from_column(field)?,
                NumericKey::new(kind).map_err(|_| InspectError::Unsupported)?,
            ),
        };
        Ok(Self {
            column,
            descending,
            kind,
        })
    }
    fn marker(&self, null: bool) -> u8 {
        match (null, self.descending) {
            (true, false) => ASC_NULL,
            (true, true) => DESC_NULL,
            (false, false) => ASC_VALUE,
            (false, true) => DESC_VALUE,
        }
    }
    fn append(
        &self,
        row: &(impl KeyRow + ?Sized),
        output: &mut Vec<u8>,
        work: &mut KeyWork<'_>,
    ) -> Result<(), InspectError> {
        check(work.budget, work.cancelled)?;
        let value = row.get(self.column).ok_or(InspectError::Malformed)?;
        let mut fixed = [0; SCALAR_BUFFER_BYTES];
        let length = match &self.kind {
            Kind::Text(field) => {
                let profile = TextProfile::new(self.column, field, self.descending)?;
                let bytes = profile.row_component(row, work)?;
                return append(output, &bytes, work);
            }
            Kind::Numeric(kind, codec) => {
                encode_component(kind.key(value)?, *codec, self.descending, &mut fixed)?
            }
            Kind::Binary(maximum) => {
                binary::encode_component(value, *maximum, self.descending, &mut fixed)?
            }
            Kind::Guid => {
                guid::encode_component(guid::value_key(value)?, self.descending, &mut fixed)?
            }
            Kind::Decimal(kind) => {
                kind.encode_read_component(value, self.descending, &mut fixed)?
            }
        };
        append(output, &fixed[..length], work)
    }
}

pub(in crate::table::indexes) struct TextTupleProfile<'a> {
    // Metadata is bounded by the native ten-column ceiling, without heap clones.
    columns: [Option<Component<'a>>; INDEX_KEY_COLUMNS],
    count: usize,
    memo: bool,
}
/// Physical equality ignores row-only NULL metadata. Decoded separator keys may
/// have been cut inside a component; they cannot supply constraint NULL facts.
pub(in crate::table::indexes) struct TextTupleKey {
    bytes: Vec<u8>,
    row_nulls: Option<u16>,
}
impl PartialEq for TextTupleKey {
    fn eq(&self, other: &Self) -> bool {
        self.bytes == other.bytes
    }
}
impl Eq for TextTupleKey {}

pub(in crate::table::indexes) fn has_text(
    index: &NativeIndex,
    schema: &TableSchema,
) -> Result<bool, InspectError> {
    let mut found = false;
    for column in &index.columns {
        let field = schema
            .columns
            .iter()
            .find(|field| field.number() == column.number())
            .ok_or(InspectError::Malformed)?;
        found |= matches!(
            field.native_type(),
            data_type::SHORT_TEXT | data_type::LONG_TEXT
        );
    }
    Ok(found)
}
impl<'a> TextTupleProfile<'a> {
    pub fn new(index: &NativeIndex, schema: &'a TableSchema) -> Result<Self, InspectError> {
        qualify(index)?;
        if !(2..=INDEX_KEY_COLUMNS).contains(&index.columns.len()) {
            return Err(InspectError::Unsupported);
        }
        let mut columns = std::array::from_fn(|_| None);
        let mut memo = false;
        let mut text = false;
        for (slot, column) in columns.iter_mut().zip(&index.columns) {
            let position = schema
                .columns
                .iter()
                .position(|field| field.number() == column.number())
                .ok_or(InspectError::Malformed)?;
            let component =
                Component::new(position, &schema.columns[position], column.descending())?;
            if let Kind::Text(field) = &component.kind {
                text = true;
                memo |= field.native_type() == data_type::LONG_TEXT;
            }
            *slot = Some(component);
        }
        if !text {
            return Err(InspectError::Unsupported);
        }
        Ok(Self {
            columns,
            count: index.columns.len(),
            memo,
        })
    }
    fn columns(&self) -> impl Iterator<Item = &Component<'_>> {
        self.columns[..self.count].iter().flatten()
    }
    fn null_mask(&self) -> u16 {
        (1 << self.count) - 1
    }
}
impl KeyProfile for TextTupleProfile<'_> {
    type Key = TextTupleKey;
    fn components(&self) -> usize {
        self.count
    }
    fn max_key_bytes(&self) -> usize {
        text::MAX_KEY_BYTES
    }
    fn decode(
        &self,
        bytes: &[u8],
        work: &mut KeyWork<'_>,
    ) -> Result<(Self::Key, usize), InspectError> {
        check(work.budget, work.cancelled)?;
        let first = self.columns().next().ok_or(InspectError::Malformed)?;
        let marker = *bytes.first().ok_or(InspectError::Malformed)?;
        if bytes.len() > text::MAX_KEY_BYTES
            || (marker != first.marker(true) && marker != first.marker(false))
        {
            return Err(InspectError::Malformed);
        }
        // Entry framing already excludes row/child trailers. Complete row-key
        // re-encoding validates every leaf component, including unprojected ones.
        let mut owned = Vec::new();
        append(&mut owned, bytes, work)?;
        Ok((
            TextTupleKey {
                bytes: owned,
                row_nulls: None,
            },
            bytes.len(),
        ))
    }
    fn compare(&self, left: &Self::Key, right: &Self::Key) -> Ordering {
        left.bytes.cmp(&right.bytes)
    }
    fn row_key(
        &self,
        row: &(impl KeyRow + ?Sized),
        work: &mut KeyWork<'_>,
    ) -> Result<Self::Key, InspectError> {
        let mut bytes = Vec::new();
        let mut nulls = 0;
        for (position, column) in self.columns().enumerate() {
            if matches!(
                row.get(column.column).ok_or(InspectError::Malformed)?,
                Value::Null
            ) {
                nulls |= 1 << position;
            }
            column.append(row, &mut bytes, work)?;
        }
        text::digest::truncate(&mut bytes, work)?;
        check(work.budget, work.cancelled)?;
        Ok(TextTupleKey {
            bytes,
            row_nulls: Some(nulls),
        })
    }

    fn constraint_nulls(&self, key: &Self::Key) -> Result<(bool, bool), InspectError> {
        let nulls = key.row_nulls.ok_or(InspectError::Malformed)?;
        Ok((nulls == self.null_mask(), nulls != 0))
    }
    fn unique_key_is_exact(&self, key: &Self::Key) -> bool {
        !self.memo && key.bytes.len() < text::MAX_KEY_BYTES
    }
}
fn append(output: &mut Vec<u8>, bytes: &[u8], work: &mut KeyWork<'_>) -> Result<(), InspectError> {
    check(work.budget, work.cancelled)?;
    work.budget.charge(Resource::Steps, bytes.len() as u64)?;
    work.budget
        .charge_allocation(bytes.len() as u64, size_of::<u8>() as u64)?;
    output
        .try_reserve_exact(bytes.len())
        .map_err(|_| InspectError::Allocation)?;
    output.extend_from_slice(bytes);
    Ok(())
}

#[cfg(test)]
mod tests;
