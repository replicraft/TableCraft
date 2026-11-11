//! Literal known native2 tail frames over the retained ID/Score fixed prefix.
use super::super::{
    exact_values,
    fixed_scalars::{self, Column, Scalar},
    typed_fields::Kind,
};
use crate::fixtures::access::{
    generated_record_insert::placement::{self, Pages},
    packed_record::append_record,
    put16, put32, raw_forwarding_chain,
};
use tablecraft_core::values::Value;

const PAGE: usize = 4096;
fn word(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize
}
fn declaration(before: &[u8], definition: u32, kind: Kind) -> bool {
    let base = definition as usize * PAGE;
    assert_eq!(word(before, base + 41), 3);
    let indexed = u32::from_le_bytes(before[base + 51..base + 55].try_into().unwrap()) as usize;
    assert!(indexed <= 1);
    assert_eq!(word(before, base + 45), if indexed == 1 { 3 } else { 2 });
    let at = base + 63 + indexed * 12 + (if indexed == 1 { 2 } else { 1 }) * 25;
    let (native, width) = kind.storage();
    assert_eq!(before[at], native);
    assert_eq!(word(before, at + 5), 2);
    assert_eq!(
        &before[at + 15..at + 17],
        &[if width.is_some() { 3 } else { 2 }, 0]
    );
    assert_eq!(word(before, base + 43), usize::from(width.is_none()));
    if let Some(width) = width {
        assert_eq!(word(before, at + 21), if width == 0 { 0 } else { 8 });
        assert_eq!(word(before, at + 23), width);
    }
    match kind {
        Kind::Decimal { precision, scale } => {
            assert_eq!(&before[at + 11..at + 13], &[precision, scale]);
            assert_eq!(word(before, at + 23), 17);
        }
        Kind::ShortText { units, compression } => {
            assert!(!compression);
            assert_eq!(word(before, at + 23), usize::from(units) * 2);
        }
        _ => {}
    }
    indexed == 1
}
fn payload(kind: Kind, value: &Value) -> Vec<u8> {
    match (kind, value) {
        (_, Value::Null) => Vec::new(),
        (Kind::Boolean, Value::Boolean(_)) => Vec::new(),
        (Kind::Byte, Value::Byte(v)) => {
            fixed_scalars::payload(Column::ByteValue, Some(Scalar::Byte(*v)))
        }
        (Kind::Int16, Value::Int16(v)) => {
            fixed_scalars::payload(Column::ShortValue, Some(Scalar::Int16(*v)))
        }
        (Kind::Int64, Value::Int64(v)) => {
            fixed_scalars::payload(Column::LargeValue, Some(Scalar::Int64(*v)))
        }
        (Kind::Single, Value::Single(v)) => {
            fixed_scalars::payload(Column::SingleValue, Some(Scalar::SingleBits(v.to_bits())))
        }
        (Kind::Double, Value::Double(v)) => {
            fixed_scalars::payload(Column::DoubleValue, Some(Scalar::DoubleBits(v.to_bits())))
        }
        (Kind::DateTime, Value::DateTime(v)) => {
            fixed_scalars::payload(Column::DateValue, Some(Scalar::DateTimeBits(v.to_bits())))
        }
        (Kind::Int32, Value::Int32(v)) => v.to_le_bytes().to_vec(),
        (Kind::Currency, Value::Currency(v)) => v.scaled().to_le_bytes().to_vec(),
        (Kind::Guid, Value::Guid(v)) => exact_values::guid_bytes(v.to_bytes()).to_vec(),
        (Kind::Decimal { precision, scale }, Value::Decimal(v)) => {
            assert_eq!(v.scale(), scale);
            assert!(v.coefficient().unsigned_abs() < 10_u128.pow(u32::from(precision)));
            exact_values::decimal_bytes(v.coefficient()).to_vec()
        }
        (
            Kind::ShortText {
                units,
                compression: false,
            },
            Value::Text(v),
        ) => {
            assert!(v.encode_utf16().count() <= usize::from(units));
            v.encode_utf16().flat_map(u16::to_le_bytes).collect()
        }
        _ => panic!("exact scalar carrier for actual declaration"),
    }
}
fn mask(value: &Value, kind: Kind) -> bool {
    match (kind, value) {
        (Kind::Boolean, Value::Boolean(value)) => *value,
        (Kind::Boolean, _) => panic!("Boolean has noNULL"),
        (_, Value::Null) => false,
        _ => true,
    }
}

/// Materialize or replace only the actual added native2 field; retained ignored
/// padding and siblings survive, and old absentNULL/false no-ops are exact.
pub fn update(before: &[u8], definition: u32, root: usize, kind: Kind, value: &Value) -> Vec<u8> {
    declaration(before, definition, kind);
    let old = raw_forwarding_chain::record(before, definition + 1, root);
    let count = word(old, 0);
    assert!(matches!(count, 2 | 3));
    let present = mask(value, kind);
    let (_, width) = kind.storage();
    if count == 2 && !present {
        return before.to_vec();
    }
    let oldmask = old[old.len() - 1];
    let body = payload(kind, value);
    let mut row;
    if let Some(width) = width {
        assert_eq!(old.len(), if count == 2 { 11 } else { 11 + width });
        row = old[..old.len() - 1].to_vec();
        row.resize(10 + width, 0);
        put16(&mut row, 0, 3);
        if present && width != 0 {
            assert_eq!(body.len(), width);
            row[10..10 + width].copy_from_slice(&body);
        }
    } else {
        let (stop, dataend) = if count == 2 {
            assert_eq!(old.len(), 11);
            (10, 10)
        } else {
            let mask = old.len() - 1;
            assert_eq!(word(old, mask - 2), 1);
            assert_eq!(word(old, mask - 4), 10);
            (word(old, mask - 6), mask - 6)
        };
        row = old[..10].to_vec();
        put16(&mut row, 0, 3);
        row.extend(body);
        let end = row.len();
        row.extend_from_slice(&old[stop..dataend]);
        row.resize(row.len().max(dataend), 0);
        row.extend((end as u16).to_le_bytes());
        row.extend(10_u16.to_le_bytes());
        row.extend(1_u16.to_le_bytes());
    }
    row.push((oldmask & !4) | if present { 4 } else { 0 });
    if row == old {
        return before.to_vec();
    }
    raw_forwarding_chain::place_terminal(before, definition + 1, root, &row)
}

/// Append explicitly resolved values; NULL is a supplied/resolved outcome, not
/// a gallery default. Generated fixtures use the actual LAST+primary owner.
pub fn append_resolved(
    before: &[u8],
    definition: u32,
    kind: Kind,
    score: Option<i32>,
    value: &Value,
) -> Vec<u8> {
    let generated = declaration(before, definition, kind);
    let pages = Pages {
        definition: definition as usize,
        data: definition as usize + 1,
        index: definition as usize + 2,
    };
    let id = if generated {
        placement::next_id(before, pages)
    } else {
        0
    };
    let mut row = vec![3, 0];
    row.extend(id.to_le_bytes());
    row.extend(score.unwrap_or(0).to_le_bytes());
    let body = payload(kind, value);
    let (_, width) = kind.storage();
    if let Some(width) = width {
        row.resize(10 + width, 0);
        if !matches!(value, Value::Null) && width != 0 {
            assert_eq!(body.len(), width);
            row[10..10 + width].copy_from_slice(&body);
        }
    } else {
        row.extend(body);
        let end = row.len();
        row.extend((end as u16).to_le_bytes());
        row.extend(10_u16.to_le_bytes());
        row.extend(1_u16.to_le_bytes());
    }
    row.push(
        u8::from(generated)
            | if score.is_some() { 2 } else { 0 }
            | if mask(value, kind) { 4 } else { 0 },
    );
    if generated {
        placement::append(before, pages, id, &row)
    } else {
        let mut after = before.to_vec();
        let base = definition as usize * PAGE;
        let data = (definition as usize + 1) * PAGE;
        append_record(&mut after[data..data + PAGE], &row);
        let live = u32::from_le_bytes(before[base + 16..base + 20].try_into().unwrap());
        put32(&mut after, base + 16, live + 1);
        after
    }
}
