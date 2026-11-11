//! Physical field framing shared by actual gallery and explicit declarations.
use super::super::{CurrentFieldAdditionError as Error, Target};
use super::{Plan, put16};
use crate::{
    NativeSortOrder,
    creation::profile::Field,
    format::{
        data_type::{INT32, SHORT_TEXT},
        layout::{self, COLUMN_ENTRY_BYTES, FIXED_FLAG, UPDATABLE_FLAG},
    },
};

const FIELD_MAGIC: u32 = 1625;
const MAGIC_OFFSET: usize = 1;
const NUMBER_OFFSET: usize = 5;
const VARIABLE_OFFSET: usize = 7;
const STORED_ID_OFFSET: usize = 9;
const FLAGS_OFFSET: usize = 15;
const FIXED_OFFSET: usize = 21;
const WIDTH_OFFSET: usize = 23;
const GENERAL_SORT_ORDER: [u8; 4] = [9, 4, 0, 1];
const SORT_ORDER_OFFSET: usize = 11;
const PRECISION_OFFSET: usize = 11;
const SCALE_OFFSET: usize = 12;

pub(super) struct Frame {
    pub bytes: [u8; COLUMN_ENTRY_BYTES],
    pub variable: bool,
}

fn header(plan: &Plan) -> Result<[u8; COLUMN_ENTRY_BYTES], Error> {
    let mut bytes = [0; COLUMN_ENTRY_BYTES];
    bytes[MAGIC_OFFSET..MAGIC_OFFSET + size_of::<u32>()]
        .copy_from_slice(&FIELD_MAGIC.to_le_bytes());
    put16(&mut bytes, NUMBER_OFFSET, plan.number)?;
    put16(&mut bytes, VARIABLE_OFFSET, plan.variable)?;
    put16(&mut bytes, STORED_ID_OFFSET, usize::from(plan.column_id))?;
    Ok(bytes)
}

pub(super) fn field(plan: &Plan, field: Field<'_>) -> Result<Frame, Error> {
    let order = (field.kind == SHORT_TEXT)
        .then_some(NativeSortOrder::from_native_bytes(GENERAL_SORT_ORDER));
    field_with_order(plan, field, order)
}

pub(super) fn field_with_order(
    plan: &Plan,
    field: Field<'_>,
    order: Option<NativeSortOrder>,
) -> Result<Frame, Error> {
    let mut bytes = header(plan)?;
    bytes[0] = field.kind;
    bytes[FLAGS_OFFSET] = UPDATABLE_FLAG
        | if field.offset.is_some() {
            FIXED_FLAG
        } else {
            0
        };
    put16(&mut bytes, FIXED_OFFSET, field.offset.unwrap_or(0))?;
    put16(&mut bytes, WIDTH_OFFSET, field.width)?;
    if field.kind == SHORT_TEXT {
        bytes[SORT_ORDER_OFFSET..FLAGS_OFFSET]
            .copy_from_slice(&order.ok_or(Error::Preservation)?.native_bytes());
        bytes[FLAGS_OFFSET + 1] = if field.unicode_compression {
            layout::UNICODE_COMPRESSION_FLAG
        } else {
            0
        };
    }
    if let Some(constraints) = field.decimal {
        bytes[PRECISION_OFFSET] = constraints.precision();
        bytes[SCALE_OFFSET] = constraints.scale();
    }
    Ok(Frame {
        bytes,
        variable: field.offset.is_none(),
    })
}

pub(super) fn target(plan: &Plan, target: Target) -> Result<Frame, Error> {
    if let Target::Typed(target) = target {
        return field(
            plan,
            target
                .field(plan.fixed_offset)
                .map_err(super::super::creation_error)?,
        );
    }
    let mut bytes = header(plan)?;
    match target {
        Target::Number => {
            bytes[0] = INT32;
            bytes[FLAGS_OFFSET] = FIXED_FLAG | UPDATABLE_FLAG;
            put16(&mut bytes, FIXED_OFFSET, plan.fixed_offset)?;
            put16(&mut bytes, WIDTH_OFFSET, size_of::<i32>())?;
        }
        Target::ShortText {
            max_utf16_units,
            unicode_compression,
        } => {
            bytes[0] = SHORT_TEXT;
            bytes[FLAGS_OFFSET] = UPDATABLE_FLAG;
            bytes[FLAGS_OFFSET + 1] = if unicode_compression {
                layout::UNICODE_COMPRESSION_FLAG
            } else {
                0
            };
            bytes[SORT_ORDER_OFFSET..FLAGS_OFFSET].copy_from_slice(&GENERAL_SORT_ORDER);
            put16(&mut bytes, WIDTH_OFFSET, max_utf16_units * size_of::<u16>())?;
        }
        Target::Typed(_) => return Err(Error::Preservation),
    }
    Ok(Frame {
        bytes,
        variable: matches!(target, Target::ShortText { .. }),
    })
}
