//! Named input intent over the actual maintained independent batch-image owner.
use super::super::model::{self, Kind, Row};
use tablecraft_core::values::Value;

pub(super) struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub original: Vec<Row>,
    pub final_values: Vec<Vec<Value>>,
    pub fields: Vec<&'static str>,
    pub requests: Vec<u8>,
    pub generated: bool,
    pub kind: Kind,
}

pub(super) fn reordered(code: u8, generated: bool, kind: Kind, count: usize) -> Case {
    let original = model::case(code, generated, kind, count);
    let fields = if generated {
        vec!["Other", "Id"]
    } else {
        vec!["Other", "Score", "Id"]
    };
    let mut requests = String::new();
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .from_reader(original.requests.as_slice());
    for record in reader.records() {
        let record = record.unwrap();
        let pairs = record.len() / 2;
        for position in (0..pairs).rev() {
            if position != pairs - 1 {
                requests.push('\t');
            }
            requests.push_str(&record[position * 2]);
            requests.push('\t');
            requests.push_str(&record[position * 2 + 1]);
        }
        requests.push('\n');
    }
    let mut rows = original.original.clone();
    rows.extend(original.added);
    Case {
        before: original.before,
        expected: original.expected,
        original: original.original,
        final_values: rows.iter().map(|row| row.values(kind)).collect(),
        fields,
        requests: requests.into_bytes(),
        generated,
        kind,
    }
}

pub(super) fn missing_other(code: u8, generated: bool, kind: Kind) -> Case {
    let original = model::case(code, generated, kind, 2);
    let added = [
        Row {
            id: 4,
            score: if generated { 31 } else { 40 },
            other: Some(default_bits(kind)),
        },
        Row {
            id: 5,
            score: if generated { 32 } else { 50 },
            other: Some(default_bits(kind)),
        },
    ];
    let expected = model::append(&original.before, generated, kind, &added);
    let fields = if generated {
        vec!["Id"]
    } else {
        vec!["Score", "Id"]
    };
    let mut requests = String::new();
    for row in &added {
        if !generated {
            requests.push_str(&format!("int32\t{}\t", row.score));
        }
        requests.push_str(&format!("int32\t{}\n", row.id));
    }
    let mut rows = original.original.clone();
    rows.extend(added);
    Case {
        before: original.before,
        expected,
        original: original.original,
        final_values: rows.iter().map(|row| row.values(kind)).collect(),
        fields,
        requests: requests.into_bytes(),
        generated,
        kind,
    }
}

pub(super) fn missing_nullable_id(code: u8, generated: bool) -> Case {
    let kind = Kind::Integer;
    let original = model::case(code, generated, kind, 2);
    let added = [
        Row {
            id: 0,
            score: if generated { 31 } else { 40 },
            other: Some(99),
        },
        Row {
            id: 0,
            score: if generated { 32 } else { 50 },
            other: None,
        },
    ];
    let mut expected = model::append(&original.before, generated, kind, &added);
    // Id is the actual nullable, unindexed native slot0. Omitted Id has zero
    // fixed payload and a cleared presence bit; it is not a supplied numeric0.
    for slot in [3, 4] {
        let at = 12 * 4096 + 14 + slot * 2;
        let start = usize::from(u16::from_le_bytes(expected[at..at + 2].try_into().unwrap()));
        assert_eq!(
            &expected[12 * 4096 + start + 2..12 * 4096 + start + 6],
            &[0; 4]
        );
        expected[12 * 4096 + start + 14] &= !1;
    }
    let fields = if generated {
        vec!["Other"]
    } else {
        vec!["Other", "Score"]
    };
    let mut requests = String::new();
    for (position, row) in added.iter().enumerate() {
        requests.push_str(if position == 0 { "omitted\t" } else { "null\t" });
        if !generated {
            requests.push_str(&format!("\tint32\t{}", row.score));
        }
        requests.push('\n');
    }
    let mut final_values = original
        .original
        .iter()
        .map(|row| row.values(kind))
        .collect::<Vec<_>>();
    for row in &added {
        let mut values = row.values(kind);
        values[0] = Value::Null;
        final_values.push(values);
    }
    Case {
        before: original.before,
        expected,
        original: original.original,
        final_values,
        fields,
        requests: requests.into_bytes(),
        generated,
        kind,
    }
}

fn default_bits(kind: Kind) -> u64 {
    match kind {
        Kind::Integer => 99,
        Kind::Clock => 0x40e1dd3000000000,
    }
}
