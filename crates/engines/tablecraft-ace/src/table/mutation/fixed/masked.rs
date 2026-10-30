use super::super::{
    Ace16WritePreview, EditError, WriteBlocker, field, gates,
    patch::BytePatch,
    rows::{self, RowAssignment},
};
use super::{FixedScalarUpdate, encoding};
use crate::{
    InspectError, NativeImage, TableCheck,
    format::{
        reader::Reader,
        row::{COLUMN_COUNT_BYTES, MASK_BITS},
    },
    header::check,
    table::indexes::{IndexKeyUpdate, MutationKey, fixed_index_patches},
};
use std::iter;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::ScalarType;

const MAX_PATCHES_PER_ROW: usize = 2;

struct Cell<const WIDTH: usize> {
    // Boolean carries its value bit here; other fields carry NULL presence.
    present: bool,
    bytes: [u8; WIDTH],
}
fn cell<const WIDTH: usize>(kind: ScalarType, value: &Value) -> Result<Cell<WIDTH>, EditError> {
    if kind == ScalarType::Boolean {
        return match value {
            Value::Boolean(present) => Ok(Cell {
                present: *present,
                bytes: [0; WIDTH],
            }),
            _ => Err(EditError::Unsupported(WriteBlocker::Column)),
        };
    }
    match value {
        Value::Null => Ok(Cell {
            present: false,
            bytes: [0; WIDTH],
        }),
        _ => Ok(Cell {
            present: true,
            bytes: encoding::encode(kind, value)?,
        }),
    }
}
struct Input<const WIDTH: usize> {
    row: usize,
    before: Cell<WIDTH>,
    after: Cell<WIDTH>,
}
impl<const WIDTH: usize> RowAssignment for Input<WIDTH> {
    fn row(&self) -> usize {
        self.row
    }
}

enum Span<const WIDTH: usize> {
    Payload([u8; WIDTH]),
    Mask(u8),
}
impl<const WIDTH: usize> Span<WIDTH> {
    fn bytes(&self) -> &[u8] {
        match self {
            Self::Payload(bytes) => bytes,
            Self::Mask(byte) => std::slice::from_ref(byte),
        }
    }
}
pub(in crate::table::mutation) struct Patch<const WIDTH: usize> {
    offset: usize,
    before: Span<WIDTH>,
    after: Span<WIDTH>,
}
impl<const WIDTH: usize> BytePatch for Patch<WIDTH> {
    fn offset(&self) -> usize {
        self.offset
    }
    fn before(&self) -> &[u8] {
        self.before.bytes()
    }
    fn after(&self) -> &[u8] {
        self.after.bytes()
    }
    fn is_mask(&self) -> bool {
        matches!(self.before, Span::Mask(_))
    }
}

pub(super) fn generate<const WIDTH: usize>(
    image: NativeImage,
    table: &str,
    column: &str,
    updates: &[FixedScalarUpdate],
    kind: ScalarType,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    let structural = gates::before_fixed_update(&image, table, budget, cancelled)?;
    super::super::data_ownership::require_index_disjointness(
        &image,
        structural.schema(),
        budget,
        cancelled,
    )?;
    super::extended::qualify(&image, &structural, iter::once(column), budget, cancelled)?;
    let staged = stage::<WIDTH, true>(
        &image,
        &structural,
        column,
        updates,
        kind,
        budget,
        cancelled,
    )?;
    let indexes = fixed_index_patches(
        &image,
        structural.schema(),
        structural.indexes(),
        &staged.keys,
        budget,
        cancelled,
    )?;
    image
        .finish_updates(
            table,
            &staged.patches,
            &indexes,
            staged.changed.len(),
            budget,
            cancelled,
        )
        .map(Ace16WritePreview::from_plan)
}

pub(in crate::table::mutation) struct ColumnEdits<const WIDTH: usize> {
    pub patches: Vec<Patch<WIDTH>>,
    pub keys: Vec<IndexKeyUpdate>,
    pub changed: Vec<u32>,
}

pub(in crate::table::mutation) fn stage<const WIDTH: usize, const OWNED_FINAL: bool>(
    image: &NativeImage,
    structural: &TableCheck,
    column: &str,
    updates: &[FixedScalarUpdate],
    kind: ScalarType,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ColumnEdits<WIDTH>, EditError> {
    let schema = structural.schema();
    let column = if kind == ScalarType::Decimal {
        super::decimal::column(schema, column)?
    } else {
        field::fixed_column(schema, column, kind, WIDTH)?
    };
    let selected = &schema.columns[column];
    budget
        .charge_allocation(updates.len() as u64, size_of::<Input<WIDTH>>() as u64)
        .map_err(InspectError::from)?;
    let mut inputs = Vec::new();
    inputs
        .try_reserve_exact(updates.len())
        .map_err(|_| InspectError::Allocation)?;
    for update in updates {
        check(budget, cancelled)?;
        if kind == ScalarType::Decimal {
            let decimal = crate::table::indexes::DecimalType::from_column(selected)?;
            decimal.value_key(update.expected(), false)?;
            decimal.value_key(update.value(), false)?;
        }
        inputs.push(Input {
            row: update.row(),
            before: cell::<WIDTH>(kind, update.expected())?,
            after: cell::<WIDTH>(kind, update.value())?,
        });
    }
    rows::unique(&inputs, budget, cancelled)?;
    let indexed = structural.indexes().iter().any(|i| {
        i.index()
            .columns()
            .iter()
            .any(|key| key.number() == selected.number())
    });
    if kind == ScalarType::Decimal {
        budget
            .charge(
                tablecraft_core::limits::Resource::Steps,
                (updates.len() * schema.layout.variable_slots) as u64,
            )
            .map_err(InspectError::from)?;
    }
    let key_kind = indexed
        .then(|| MutationKey::from_column(selected))
        .transpose()?;
    let key_capacity = if indexed { updates.len() } else { 0 };
    budget
        .charge_allocation(key_capacity as u64, size_of::<IndexKeyUpdate>() as u64)
        .map_err(InspectError::from)?;
    let mut keys = Vec::new();
    keys.try_reserve_exact(key_capacity)
        .map_err(|_| InspectError::Allocation)?;
    let capacity = updates
        .len()
        .checked_mul(MAX_PATCHES_PER_ROW)
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge_allocation(capacity as u64, size_of::<Patch<WIDTH>>() as u64)
        .map_err(InspectError::from)?;
    let mut patches = Vec::new();
    patches
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;
    budget
        .charge_allocation(updates.len() as u64, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    let mut changed = Vec::new();
    changed
        .try_reserve_exact(updates.len())
        .map_err(|_| InspectError::Allocation)?;
    let visitor = |row: rows::Row<'_>, input: &Input<WIDTH>| {
        let mask_offset = row.mask + selected.number() / MASK_BITS;
        let before = *row.data.get(mask_offset).ok_or(InspectError::Malformed)?;
        let bit = 1_u8 << (selected.number() % MASK_BITS);
        if (before & bit != 0) != input.before.present {
            return Err(EditError::Conflict);
        }
        if !input.before.present && !input.after.present {
            return Ok(());
        }
        let payload_offset = if kind == ScalarType::Decimal {
            let span = super::decimal::span(schema, column, row.data, row.mask)?;
            if span.len() != WIDTH {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            span.start
        } else {
            COLUMN_COUNT_BYTES + selected.field.offset
        };
        let stored = if WIDTH != 0 {
            if payload_offset + WIDTH > row.mask {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            let data = row.data[payload_offset..payload_offset + WIDTH]
                .try_into()
                .map_err(|_| InspectError::Malformed)?;
            if input.before.present && data != input.before.bytes {
                return Err(EditError::Conflict);
            }
            data
        } else {
            [0; WIDTH]
        };
        let modified = input.before.present != input.after.present
            || (WIDTH != 0 && input.after.present && input.before.bytes != input.after.bytes);
        if !modified {
            return Ok(());
        }
        if let Some(key_kind) = key_kind {
            keys.push(IndexKeyUpdate {
                reference: row.reference,
                column,
                expected: key_kind.cell(&input.before.bytes, input.before.present)?,
                value: key_kind.cell(&input.after.bytes, input.after.present)?,
            });
        }
        if WIDTH != 0 && input.after.present && stored != input.after.bytes {
            patches.push(Patch {
                offset: row.offset + payload_offset,
                before: Span::Payload(stored),
                after: Span::Payload(input.after.bytes),
            });
        }
        let after = (before & !bit) | if input.after.present { bit } else { 0 };
        if (before ^ after) & !bit != 0 {
            return Err(EditError::Preservation);
        }
        if before != after {
            patches.push(Patch {
                offset: row.offset + mask_offset,
                before: Span::Mask(before),
                after: Span::Mask(after),
            });
        }
        changed.push(row.reference);
        Ok(())
    };
    if OWNED_FINAL {
        rows::final_record::visit(image, schema, &inputs, budget, cancelled, visitor)?;
    } else {
        let mut source = image.reader();
        rows::visit(
            &mut Reader {
                source: &mut source,
                header: image.catalog().header(),
                budget,
                cancelled,
                reads: 0,
            },
            schema,
            &inputs,
            visitor,
        )?;
    }
    Ok(ColumnEdits {
        patches,
        keys,
        changed,
    })
}
