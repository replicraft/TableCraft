//! Original-only wide scalar INSERT images. No creator, planner or encoder imports.
use super::{
    canonical_fixed_creation as fixed, canonical_native17_creation as native17,
    canonical_variable_creation as variable,
};
use tablecraft_core::values::{Currency, Decimal, Double, Guid, LegacyDateTime, Single, Value};

#[derive(Clone, Copy)]
pub enum Case {
    FixedExact,
    FixedNull,
    FixedSubnormal,
    FixedSpecial,
    DecimalExact,
    DecimalMax,
    DecimalNull,
    EofFixed,
}

pub const CASES: [Case; 8] = [
    Case::FixedExact,
    Case::FixedNull,
    Case::FixedSubnormal,
    Case::FixedSpecial,
    Case::DecimalExact,
    Case::DecimalMax,
    Case::DecimalNull,
    Case::EofFixed,
];

impl Case {
    pub const fn id(self) -> &'static str {
        match self {
            Self::FixedExact => "fixed-exact",
            Self::FixedNull => "fixed-null",
            Self::FixedSubnormal => "fixed-subnormal",
            Self::FixedSpecial => "fixed-special",
            Self::DecimalExact => "decimal-exact",
            Self::DecimalMax => "decimal-max",
            Self::DecimalNull => "decimal-null",
            Self::EofFixed => "eof-fixed",
        }
    }
}

pub struct Source {
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub values: Vec<Value>,
    pub rows_before: Vec<Vec<Value>>,
    pub columns: Vec<&'static str>,
}

fn short(raw: &[u8], offset: usize) -> usize {
    usize::from(u16::from_le_bytes(
        raw[offset..offset + 2].try_into().unwrap(),
    ))
}

fn put16(raw: &mut [u8], offset: usize, value: usize) {
    raw[offset..offset + 2].copy_from_slice(&u16::try_from(value).unwrap().to_le_bytes());
}

fn put32(raw: &mut [u8], offset: usize, value: usize) {
    raw[offset..offset + 4].copy_from_slice(&u32::try_from(value).unwrap().to_le_bytes());
}

fn fixed_values(case: Case) -> Vec<Value> {
    if matches!(case, Case::FixedNull) {
        let mut values = vec![Value::Boolean(false)];
        values.extend((0..9).map(|_| Value::Null));
        return values;
    }
    let (single, double, date) = match case {
        Case::FixedSubnormal => (1, 0x000f_ffff_ffff_ffff, 0),
        Case::FixedSpecial => (0x7f81_2345, 0xfff8_1234_5678_9abc, 0),
        _ => (0x8000_0000, 0x8000_0000_0000_0000, 0x8000_0000_0000_0000),
    };
    vec![
        Value::Boolean(true),
        Value::Byte(255),
        Value::Int16(-32768),
        Value::Int32(17),
        Value::Int64(-7),
        Value::Currency(Currency::from_scaled(-10001)),
        Value::Single(Single::from_bits(single)),
        Value::Double(Double::from_bits(double)),
        Value::DateTime(LegacyDateTime::from_bits(date).unwrap()),
        Value::Guid(Guid::from_bytes([
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ])),
    ]
}

fn fixed_record(case: Case) -> Vec<u8> {
    let mut raw = vec![0; 63];
    put16(&mut raw, 0, 10);
    if matches!(case, Case::FixedNull) {
        return raw;
    }
    raw[2] = 255;
    raw[3..5].copy_from_slice(&(-32768_i16).to_le_bytes());
    raw[5..9].copy_from_slice(&17_i32.to_le_bytes());
    raw[9..17].copy_from_slice(&(-7_i64).to_le_bytes());
    raw[17..25].copy_from_slice(&(-10001_i64).to_le_bytes());
    let (single, double, date) = match case {
        Case::FixedSubnormal => (1_u32, 0x000f_ffff_ffff_ffff_u64, 0_u64),
        Case::FixedSpecial => (0x7f81_2345, 0xfff8_1234_5678_9abc, 0),
        _ => (0x8000_0000, 0x8000_0000_0000_0000, 0x8000_0000_0000_0000),
    };
    raw[25..29].copy_from_slice(&single.to_le_bytes());
    raw[29..37].copy_from_slice(&double.to_le_bytes());
    raw[37..45].copy_from_slice(&date.to_le_bytes());
    // Explicit native GUID bytes, independent of the producer's conversion.
    raw[45..61].copy_from_slice(&[
        0x33, 0x22, 0x11, 0x00, 0x55, 0x44, 0x77, 0x66, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        0xff,
    ]);
    raw[61..63].copy_from_slice(&[0xff, 3]);
    raw
}

fn decimal_values(case: Case) -> Vec<Value> {
    match case {
        Case::DecimalExact => vec![
            Value::Int32(99),
            Value::Text("new".into()),
            Value::Binary(vec![0xc7, 0, 0xff]),
            Value::Decimal(Decimal::new(-12345, 4).unwrap()),
            Value::Boolean(true),
        ],
        Case::DecimalMax => vec![
            Value::Int32(100),
            Value::Null,
            Value::Null,
            Value::Decimal(Decimal::new(9_999_999_999_999_999_999_999_999_999, 4).unwrap()),
            Value::Boolean(false),
        ],
        Case::DecimalNull => vec![
            Value::Int32(101),
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Boolean(false),
        ],
        _ => unreachable!(),
    }
}

fn decimal_record(case: Case) -> Vec<u8> {
    match case {
        Case::DecimalExact => {
            let mut raw = vec![5, 0, 99, 0, 0, 0];
            raw.extend_from_slice(&[b'n', 0, b'e', 0, b'w', 0]);
            raw.extend_from_slice(&[0xc7, 0, 0xff]);
            raw.extend_from_slice(&[0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x39, 0x30, 0, 0]);
            raw.extend_from_slice(&[32, 0, 15, 0, 12, 0, 6, 0, 3, 0, 0x1f]);
            assert_eq!(raw.len(), 43);
            raw
        }
        Case::DecimalMax => {
            let mut raw = vec![5, 0, 100, 0, 0, 0];
            // Same literal 28-digit original magnitude, independently authored
            // by the immutable DecimalSigns source; no previous AFTER is used.
            raw.extend_from_slice(&[
                0, 0, 0, 0, 0, 0x5e, 0xce, 0x4f, 0x20, 0x61, 0x02, 0x25, 0x3e, 0xff, 0xff, 0xff,
                0x0f,
            ]);
            raw.extend_from_slice(&[23, 0, 6, 0, 6, 0, 6, 0, 3, 0, 9]);
            assert_eq!(raw.len(), 34);
            raw
        }
        Case::DecimalNull => vec![5, 0, 101, 0, 0, 0, 6, 0, 6, 0, 6, 0, 6, 0, 3, 0, 1],
        _ => unreachable!(),
    }
}

fn append(before: &[u8], record: &[u8]) -> Vec<u8> {
    let mut after = before.to_vec();
    let data = 8 * 4096;
    let count = short(before, data + 12);
    let end = short(before, data + 14 + 2 * (count - 1));
    let start = end.checked_sub(record.len()).unwrap();
    assert!(start >= 14 + 2 * (count + 1));
    put16(&mut after, data + 12, count + 1);
    put16(&mut after, data + 14 + 2 * count, start);
    put16(&mut after, data + 2, start - 14 - 2 * (count + 1));
    after[data + start..data + end].copy_from_slice(record);
    put32(&mut after, 7 * 4096 + 16, count + 1);
    after
}

pub fn source(case: Case) -> Source {
    let decimal = matches!(
        case,
        Case::DecimalExact | Case::DecimalMax | Case::DecimalNull
    );
    let eof = matches!(case, Case::EofFixed);
    let (mut before, rows_before, columns) = if decimal {
        let original = variable::source(variable::Case::DecimalSigns);
        (
            native17::source(original.bytes),
            original.rows,
            original.columns.iter().map(|column| column.name).collect(),
        )
    } else {
        let original = fixed::source(if eof {
            fixed::Case::Boundary
        } else {
            fixed::Case::SignedZero
        });
        (
            native17::source(original.bytes),
            original.rows,
            original.columns.iter().map(|column| column.name).collect(),
        )
    };
    let values = if decimal {
        decimal_values(case)
    } else {
        fixed_values(case)
    };
    let record = if decimal {
        decimal_record(case)
    } else {
        fixed_record(case)
    };
    let after = if eof {
        assert_eq!(before.len(), 36864);
        assert_eq!(short(&before, 8 * 4096 + 2), 52);
        let global = 4096 + short(&before, 4096 + 14);
        let owned = 3 * 4096 + short(&before, 3 * 4096 + 14 + 2 * 3);
        let candidate = 3 * 4096 + short(&before, 3 * 4096 + 14 + 2 * 7);
        for offset in [global, owned, candidate] {
            assert_eq!(&before[offset..offset + 5], &[0; 5]);
            assert_eq!(before[offset + 6] & 2, 0);
        }
        before[global + 6] |= 2;
        let mut after = before.clone();
        after.extend_from_slice(&[0; 4096]);
        let data = 9 * 4096;
        after[data..data + 2].copy_from_slice(&[1, 1]);
        put32(&mut after, data + 4, 7);
        put16(&mut after, data + 2, 4017);
        put16(&mut after, data + 12, 1);
        put16(&mut after, data + 14, 4033);
        after[data + 4033..data + 4096].copy_from_slice(&record);
        put32(&mut after, 7 * 4096 + 16, 63);
        after[global + 6] &= !2;
        after[owned + 6] |= 2;
        after[candidate + 6] |= 2;
        after
    } else {
        append(&before, &record)
    };
    Source {
        before,
        after,
        values,
        rows_before,
        columns,
    }
}
