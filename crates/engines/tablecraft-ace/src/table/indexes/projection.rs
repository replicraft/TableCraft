//! Explicit indexed output columns and internal key-validation projection.
use super::{
    BooleanIndexSelection, DateTimeIndexSelection, DecimalIndexSelection, Int32IndexSelection,
    Int64IndexSelection, NativeIndex, key::KeyRow,
};
use crate::{InspectError, TableFeatures, TableSchema, header::check, table::snapshot};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::Column;

/// Width-qualified selection for an explicitly projected native index read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexProjectionSelection<'a> {
    /// Complete traversal of any qualified index profile.
    All,
    /// Only a qualified single-Boolean index, with exact equality and no NULL.
    Boolean(BooleanIndexSelection),
    /// Only a qualified single-Byte index, including All/Null.
    Byte(super::ByteIndexSelection),
    /// Only a qualified single-Int16 index, including All/Null.
    Int16(super::Int16IndexSelection),
    /// Only a qualified single-Int32 index; All/Null do not relax the width.
    Int32(Int32IndexSelection),
    /// Only a qualified single-Int64 index; exact integral bounds are retained.
    Int64(Int64IndexSelection),
    /// Only a qualified single Currency index; bounds retain exact scaled values.
    Currency(super::CurrencyIndexSelection),
    /// Only a qualified single Decimal index, using exact declared equality or NULL.
    Decimal(DecimalIndexSelection),
    /// Only a qualified single legacy DATE index, with conservative calendar candidates.
    DateTime(DateTimeIndexSelection),
    /// Only a qualified single GUID index, using exact canonical equality or NULL.
    Guid(super::guid::GuidIndexSelection),
    /// Only a qualified single Short Binary index, with borrowed byte bounds.
    Binary(super::binary::BinaryIndexSelection<'a>),
    /// Only a qualified General Short Text index, with physical cursor bounds.
    Text(super::text::TextIndexSelection<'a>),
}
/// Explicit output fields and selection; eligibility is checked by the reader.
/// Output stays in native logical column order. Key fields not requested for
/// output are still decoded and checked without being exposed as output cells.
pub struct IndexProjection<'a> {
    /// Ordinary scalar field names. Empty selects zero-width rows, not zero records.
    pub columns: &'a [&'a str],
    /// Native index selection, without SQL coercion or automatic fallback.
    pub selection: IndexProjectionSelection<'a>,
}

/// Explicit complete/projected indexed scalar read and retained-row ceiling.
/// The entry point validates the ceiling before I/O. Selection qualifiers and
/// native structural/resource checks remain independent of this consumer limit.
/// Referenced rows use internal batches with separate physical-read quotas and
/// one cumulative caller budget; no partial batch/snapshot is published.
pub struct IndexReadRequest<'a> {
    /// Ordinary scalar names, or None for all scalar fields. Empty selects zero-width rows.
    pub columns: Option<&'a [&'a str]>,
    /// Native typed selection; unsupported keys fail without fallback.
    pub selection: IndexProjectionSelection<'a>,
    /// Positive retained-row ceiling, at most TableSnapshotLimits::MAX_ROWS.
    pub maximum_rows: u32,
}

pub(super) struct RowProjection {
    pub output_columns: Vec<Column>,
    decoded: Option<Decoded>,
}
struct Decoded {
    positions: Vec<usize>,
    columns: Vec<Column>,
    returned: Vec<usize>,
}
pub(super) struct KeyView<'a> {
    values: &'a [Value],
    positions: Option<&'a [usize]>,
}
impl KeyRow for KeyView<'_> {
    fn get(&self, column: usize) -> Option<&Value> {
        let dense = match self.positions {
            Some(positions) => positions.binary_search(&column).ok()?,
            None => column,
        };
        self.values.get(dense)
    }
}
impl RowProjection {
    pub fn new(
        schema: &TableSchema,
        index: &NativeIndex,
        requested: Option<&[&str]>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        Self::build(schema, index, requested, None, budget, cancelled)
    }

    pub fn stored(
        features: &TableFeatures,
        index: &NativeIndex,
        requested: Option<&[&str]>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        Self::build(
            features.schema(),
            index,
            requested,
            Some(features),
            budget,
            cancelled,
        )
    }

    fn build(
        schema: &TableSchema,
        index: &NativeIndex,
        requested: Option<&[&str]>,
        features: Option<&TableFeatures>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        if requested.is_none() && features.is_none() {
            return Ok(Self {
                output_columns: snapshot::columns(schema, budget)?,
                decoded: None,
            });
        }
        let output_count = requested.map_or(schema.columns().len(), <[&str]>::len);
        if output_count > schema.columns().len() {
            return Err(InspectError::Malformed);
        }
        let count = schema.columns().len();
        budget.charge_allocation(
            count as u64,
            (2 * size_of::<usize>() + 2 * size_of::<Column>()) as u64,
        )?;
        let mut output_columns = Vec::new();
        let mut columns = Vec::new();
        let mut positions = Vec::new();
        let mut returned = Vec::new();
        output_columns
            .try_reserve_exact(count)
            .map_err(|_| InspectError::Allocation)?;
        columns
            .try_reserve_exact(count)
            .map_err(|_| InspectError::Allocation)?;
        positions
            .try_reserve_exact(count)
            .map_err(|_| InspectError::Allocation)?;
        returned
            .try_reserve_exact(count)
            .map_err(|_| InspectError::Allocation)?;
        for (native, field) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            budget.charge(
                Resource::Steps,
                (requested.map_or(0, <[&str]>::len) + index.columns().len()) as u64,
            )?;
            let output = requested.is_none_or(|names| {
                names
                    .iter()
                    .any(|name| field.name().eq_ignore_ascii_case(name))
            });
            let key = index
                .columns()
                .iter()
                .any(|key| key.number() == field.number());
            if output || key {
                // Index keys retain ordinary scalar authority even when the same
                // field is requested as output. Calculated keys remain refused.
                let feature = if key {
                    None
                } else {
                    match features {
                        Some(features) => {
                            budget.charge(Resource::Steps, native as u64 + 1)?;
                            Some(
                                features
                                    .columns()
                                    .nth(native)
                                    .ok_or(InspectError::Malformed)?,
                            )
                        }
                        None => None,
                    }
                };
                let mut column = || match &feature {
                    Some(feature) => feature.stored_column(budget),
                    None => {
                        budget.charge_allocation(field.name().len() as u64, 1)?;
                        field.column()
                    }
                };
                let decoded = column()?;
                if output {
                    output_columns.push(column()?);
                    returned.push(positions.len());
                }
                positions.push(native);
                columns.push(decoded);
            }
        }
        if output_columns.len() != output_count {
            return Err(InspectError::Malformed);
        }
        Ok(Self {
            output_columns,
            decoded: Some(Decoded {
                positions,
                columns,
                returned,
            }),
        })
    }
    pub fn positions(&self) -> Option<&[usize]> {
        self.decoded.as_ref().map(|data| data.positions.as_slice())
    }
    pub fn columns(&self) -> &[Column] {
        self.decoded
            .as_ref()
            .map_or(&self.output_columns, |data| &data.columns)
    }
    pub fn key_view<'a>(&'a self, values: &'a [Value]) -> KeyView<'a> {
        KeyView {
            values,
            positions: self.positions(),
        }
    }
    pub fn finish(&self, values: &mut Vec<Value>) {
        if let Some(data) = &self.decoded {
            let mut position = 0;
            values.retain(|_| {
                let output = data.returned.binary_search(&position).is_ok();
                position += 1;
                output
            });
        }
    }
}
