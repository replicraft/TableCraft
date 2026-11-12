//! Literal fixed numeric carriers with protected LONG/Currency/Unicode companions.
use super::{Column, append};

/// Independently packed records and their exact source ROW references.
pub struct Source {
    pub bytes: Vec<u8>,
    pub references: Vec<u32>,
}

/// Append a manual ordinary numeric field and three non-generated companions.
/// `values` carries native integer/scaled-Currency values or literal IEEE bits.
/// Boolean is only Some(-1) or Some(0); NULL is not a Boolean storage state.
pub fn source(before: &[u8], table: &str, native: u8, values: &[Option<i64>]) -> Source {
    let width = match native {
        1 => 0,
        2 => 1,
        3 => 2,
        4 | 6 => 4,
        5 | 7 | 8 | 19 => 8,
        _ => panic!("independent numeric field kind"),
    };

    let columns = [
        Column {
            name: "KeyValue",
            native,
            width,
            variable: None,
        },
        Column {
            name: "Guard",
            native: 4,
            width: 4,
            variable: None,
        },
        Column {
            name: "Amount",
            native: 5,
            width: 8,
            variable: None,
        },
        Column {
            name: "Label",
            native: 10,
            width: 14,
            variable: Some(0),
        },
    ];

    let records = values
        .iter()
        .enumerate()
        .map(|(ordinal, value)| {
            if native == 1 {
                assert!(matches!(value, Some(-1 | 0)), "Boolean is never NULL");
            }
            let mut row = vec![4, 0];
            let value = value.unwrap_or(0);
            match native {
                1 => {}
                2 => row.push(u8::try_from(value).expect("literal Byte")),
                3 => row.extend(i16::try_from(value).expect("literal Integer").to_le_bytes()),
                4 => row.extend(i32::try_from(value).expect("literal LONG").to_le_bytes()),
                6 => {
                    let bits = u32::try_from(value).expect("literal IEEE Single bits");
                    assert!(f32::from_bits(bits).is_finite());
                    row.extend(bits.to_le_bytes());
                }
                7 | 8 => {
                    assert!(f64::from_bits(value as u64).is_finite());
                    row.extend(value.to_le_bytes());
                }
                5 | 19 => row.extend(value.to_le_bytes()),
                _ => unreachable!(),
            }
            row.extend(((ordinal % 31) as i32 - 15).to_le_bytes());
            let amount = if ordinal.is_multiple_of(2) {
                12_345_i64
            } else {
                -99_000
            };
            row.extend(amount.to_le_bytes());
            let fixed_end = row.len();
            if !ordinal.is_multiple_of(3) {
                row.extend("保護🦀".encode_utf16().flat_map(u16::to_le_bytes));
            }
            let end = row.len();
            for offset in [end, fixed_end, 1] {
                row.extend((offset as u16).to_le_bytes());
            }
            let key_present = if native == 1 {
                value == -1
            } else {
                values[ordinal].is_some()
            };
            row.push(u8::from(key_present) | 2 | 4 | (u8::from(!ordinal.is_multiple_of(3)) << 3));
            row
        })
        .collect::<Vec<_>>();

    let definition = before.len() / 4096;
    let bytes = append(before, table, &columns, &records, 21 + usize::from(width));
    let mut references = Vec::new();
    let mut page = definition + 1;
    while references.len() < values.len() {
        let start = page * 4096;
        assert_eq!(&bytes[start..start + 2], &[1, 1]);
        assert_eq!(
            u32::from_le_bytes(bytes[start + 4..start + 8].try_into().unwrap()),
            definition as u32
        );
        let count = u16::from_le_bytes(bytes[start + 12..start + 14].try_into().unwrap());
        assert!((1..=255).contains(&count));
        for slot in 0..count {
            references.push(((page as u32) << 8) | u32::from(slot));
        }
        page += 1;
    }
    assert_eq!(references.len(), values.len());

    Source { bytes, references }
}
