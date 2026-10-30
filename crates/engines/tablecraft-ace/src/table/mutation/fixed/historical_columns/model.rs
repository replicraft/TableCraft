//! Authored full-width rows retain deleted storage without inventing its types.
use super::base;
use tablecraft_core::values::Value;
use tablecraft_testkit::access as raw;

pub(super) const PAGE: usize = 4096;

pub(super) struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub rows: Vec<Vec<Value>>,
    pub final_rows: Vec<Vec<Value>>,
    pub variable: bool,
    pub selected_number: usize,
}

fn wire(value: Option<i32>, row: usize, descending: bool) -> Vec<u8> {
    let mut bytes = vec![match (value, descending) {
        (None, false) => 0,
        (None, true) => 0xff,
        (Some(_), false) => 0x7f,
        (Some(_), true) => 0x80,
    }];
    if let Some(value) = value {
        bytes.extend(
            ((value as u32) ^ 0x80000000)
                .to_be_bytes()
                .map(|byte| if descending { !byte } else { byte }),
        );
    }
    bytes.extend(((12_u32 << 8) | row as u32).to_be_bytes());
    bytes
}

fn leaf(original: &[u8], values: &[Option<i32>; 3], descending: bool) -> Vec<u8> {
    let mut page = original.to_vec();
    page[27..480].fill(0);
    let mut entries = values
        .iter()
        .enumerate()
        .map(|(row, &value)| wire(value, row, descending))
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        left[..left.len() - 4]
            .cmp(&right[..right.len() - 4])
            .then(left[left.len() - 4..].cmp(&right[right.len() - 4..]))
    });

    let mut end = 0;
    for entry in entries {
        page[480 + end..480 + end + entry.len()].copy_from_slice(&entry);
        end += entry.len();
        page[27 + end / 8] |= 1 << (end % 8);
    }
    raw::put16(&mut page, 2, (3616 - end) as u16);
    raw::put16(&mut page, 24, 0);
    page
}

fn record(row: usize, score: Option<i32>, variable: bool, selected_number: usize) -> Vec<u8> {
    let count = if variable || selected_number == 4 {
        5_u16
    } else {
        3_u16
    };
    let mut record = count.to_le_bytes().to_vec();
    record.extend((row as i32 + 1).to_le_bytes());
    // A removed fixed field occupies bytes 6..10; all four bytes are opaque.
    record.extend([0xa1 + row as u8, 0x00, 0xfe, 0x7f]);
    record.extend(score.unwrap_or(0x5a5a_5a5a).to_le_bytes());

    if variable {
        let start = record.len() as u16;
        // Variable slot zero belongs to a deleted native column. Its payload
        // includes invalid text bytes and is never interpreted as a live field.
        record.extend([0x00, 0xff, 0xd8, row as u8, 0x81]);
        let middle = record.len() as u16;
        record.extend([b'A' + row as u8, 0]);
        let end = record.len() as u16;
        for offset in [end, middle, start] {
            record.extend(offset.to_le_bytes());
        }
        record.extend(2_u16.to_le_bytes());
    }

    let score_bit = 1 << selected_number;
    let label_bit = if variable {
        1 << if selected_number == 4 { 2 } else { 4 }
    } else {
        0
    };
    let live_bits = 1 | score_bit | label_bit;
    let deleted_bits = if row == 1 {
        0
    } else {
        ((1_u8 << count) - 1) & !live_bits
    };
    record.push(1 | deleted_bits | if score.is_some() { score_bit } else { 0 } | label_bit);
    record
}

pub(super) fn case(
    code: u8,
    descending: bool,
    variable: bool,
    nullable: bool,
    selected_number: usize,
) -> Case {
    let values = [Some(10), if nullable { None } else { Some(20) }, Some(30)];
    let final_values: [Option<i32>; 3] = [Some(17), Some(25), Some(30)];
    let validation_rule = if nullable {
        "[Score] Is Null Or ([Score]>=0 And [Score]<=100)"
    } else {
        ">=0 And <=100"
    };
    let mut before = base::with_rule("ValidationRule", validation_rule);
    before.resize(15 * PAGE, 0);
    before[4..20].copy_from_slice(b"Standard ACE DB\0");
    before[20] = code;

    let fields = if variable {
        vec![("Id", 4, 0, 4), ("Score", 4, 8, 4), ("Label", 10, 0, 32)]
    } else {
        vec![("Id", 4, 0, 4), ("Score", 4, 8, 4)]
    };
    let mut definition = base::definition(&fields, 3, 11 << 8);
    raw::put16(
        &mut definition,
        41,
        if variable || selected_number == 4 {
            5
        } else {
            3
        },
    );
    raw::put16(&mut definition, 63 + 25 + 5, selected_number as u16);
    if variable {
        raw::put16(&mut definition, 43, 2);
        raw::put16(
            &mut definition,
            63 + 50 + 5,
            if selected_number == 4 { 2 } else { 4 },
        );
        raw::put16(&mut definition, 63 + 50 + 7, 1);
    }

    let names_end = 63
        + fields.len() * 25
        + fields
            .iter()
            .map(|field| 2 + field.0.len() * 2)
            .sum::<usize>();
    definition.copy_within(63..names_end, 75);
    definition[63..75].fill(0);
    raw::put32(&mut definition, 67, 29);
    raw::put32(&mut definition, 47, 2);
    raw::put32(&mut definition, 51, 1);
    let physical = names_end + 12;
    for component in 0..10 {
        raw::put16(&mut definition, physical + 4 + component * 3, 0xffff);
    }
    raw::put16(&mut definition, physical + 4, selected_number as u16);
    definition[physical + 6] = u8::from(!descending);
    raw::put32(&mut definition, physical + 34, 14 << 8);
    raw::put32(&mut definition, physical + 38, 13);
    definition[physical + 46] = 0x81;
    let logical = physical + 52;
    raw::put32(&mut definition, logical + 28 + 4, 1);
    let mut names = logical + 56;
    for name in ["ScoreKey", "ScoreAlias"] {
        let encoded = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        raw::put16(&mut definition, names, encoded.len() as u16);
        names += 2;
        definition[names..names + encoded.len()].copy_from_slice(&encoded);
        names += encoded.len();
    }
    before[10 * PAGE..11 * PAGE].copy_from_slice(&definition);

    let records = values
        .iter()
        .enumerate()
        .map(|(row, &value)| (0, record(row, value, variable, selected_number)))
        .collect::<Vec<_>>();
    before[12 * PAGE..13 * PAGE].copy_from_slice(&raw::data_page(10, &records));
    before[14 * PAGE..15 * PAGE]
        .copy_from_slice(&raw::data_page(1, &[(0, vec![0, 0, 0, 0, 0, 0, 0x20])]));
    let mut original_leaf = vec![0xa7; PAGE];
    original_leaf[..480].fill(0);
    original_leaf[..2].copy_from_slice(&[4, 1]);
    raw::put32(&mut original_leaf, 4, 10);
    before[13 * PAGE..14 * PAGE].copy_from_slice(&leaf(&original_leaf, &values, descending));

    // Expected record edits change only the actual live native-number-two cell.
    let mut expected = before.clone();
    for (row, value) in final_values.iter().enumerate().take(2) {
        let slot = 12 * PAGE + 14 + row * 2;
        let start = usize::from(u16::from_le_bytes(
            before[slot..slot + 2].try_into().unwrap(),
        ));
        let length = records[row].1.len();
        expected[12 * PAGE + start + 10..12 * PAGE + start + 14]
            .copy_from_slice(&value.unwrap().to_le_bytes());
        expected[12 * PAGE + start + length - 1] |= 1 << selected_number;
    }
    expected[13 * PAGE..14 * PAGE].copy_from_slice(&leaf(
        &before[13 * PAGE..14 * PAGE],
        &final_values,
        descending,
    ));

    // Native row APIs expose surviving fields in ascending native-number order,
    // independently of the original TD declaration-block order retained above.
    let rows = |values: &[Option<i32>; 3]| {
        values
            .iter()
            .enumerate()
            .map(|(row, &score)| {
                let mut columns = vec![
                    (0, Value::Int32(row as i32 + 1)),
                    (selected_number, score.map_or(Value::Null, Value::Int32)),
                ];
                if variable {
                    let number = if selected_number == 4 { 2 } else { 4 };
                    columns.push((
                        number,
                        Value::Text(char::from(b'A' + row as u8).to_string()),
                    ));
                }
                columns.sort_by_key(|(number, _)| *number);
                columns.into_iter().map(|(_, value)| value).collect()
            })
            .collect()
    };

    Case {
        before,
        expected,
        rows: rows(&values),
        final_rows: rows(&final_values),
        variable,
        selected_number,
    }
}
