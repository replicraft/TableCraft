//! New complete C5/V3 images authored from original rows and immutable requests.
#[path = "model/keys.rs"]
mod keys;
#[path = "model/pages.rs"]
mod pages;
#[path = "model/records.rs"]
mod records;
#[path = "model/schema.rs"]
mod schema;

use super::{base, frames};
use tablecraft_core::values::Value;

pub const PAGE: usize = 4096;

#[derive(Clone, Copy, Debug)]
pub enum Family {
    LegacyText,
    LegacyBinary,
    ModernBinary,
    Binary,
}

impl Family {
    fn first_text(self) -> bool {
        !matches!(self, Self::Binary)
    }

    fn second_text(self) -> bool {
        matches!(self, Self::LegacyText)
    }

    fn first_key(self) -> frames::Family {
        match self {
            Self::LegacyText | Self::LegacyBinary => frames::Family::Legacy,
            Self::ModernBinary => frames::Family::Modern,
            Self::Binary => frames::Family::Binary,
        }
    }

    fn second_key(self) -> frames::Family {
        if self.second_text() {
            frames::Family::Legacy
        } else {
            frames::Family::Binary
        }
    }

    fn low(self, first: bool) -> Vec<u8> {
        let text = if first {
            self.first_text()
        } else {
            self.second_text()
        };
        vec![if text { b'a' } else { 0 }]
    }

    fn high(self, first: bool) -> Vec<u8> {
        let text = if first {
            self.first_text()
        } else {
            self.second_text()
        };
        vec![if text { b's' } else { 255 }]
    }

    fn long(self, first: bool) -> Vec<u8> {
        let text = if first {
            self.first_text()
        } else {
            self.second_text()
        };
        if text { b"aa".to_vec() } else { vec![0, 255] }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Recipe {
    Permutation,
    NullShrinkGrow,
}

#[derive(Clone, Copy, Debug)]
pub enum Placement {
    Fitted,
    Data,
    DataIndexes,
}

#[derive(Clone, Copy)]
enum Serialization {
    Fitted,
    Growing,
}

#[derive(Clone)]
pub struct Row {
    pub id: i32,
    pub age: Option<i32>,
    pub first: Option<Vec<u8>>,
    pub second: Option<Vec<u8>>,
    pub opaque: Option<Vec<u8>>,
}

pub struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub rows: Vec<Row>,
    pub final_rows: Vec<Row>,
    pub records: Vec<Vec<u8>>,
    pub final_records: Vec<Vec<u8>>,
    pub family: Family,
    pub placement: Placement,
    pub first_rows: Vec<usize>,
    pub second_rows: Vec<usize>,
    pub fixed_rows: Vec<usize>,
    pub changed: Vec<usize>,
    pub data_pages: Vec<u32>,
    pub index_pages: Vec<u32>,
    pub fitted_refuses: bool,
}

pub fn values(row: &Row, family: Family) -> Vec<Value> {
    vec![
        Value::Int32(row.id),
        row.age.map_or(Value::Null, Value::Int32),
        variable(row.first.as_deref(), family.first_text()),
        variable(row.second.as_deref(), family.second_text()),
        row.opaque
            .as_ref()
            .map_or(Value::Null, |bytes| Value::Binary(bytes.clone())),
    ]
}

fn variable(value: Option<&[u8]>, text: bool) -> Value {
    value.map_or(Value::Null, |bytes| {
        if text {
            Value::Text(String::from_utf8(bytes.to_vec()).unwrap())
        } else {
            Value::Binary(bytes.to_vec())
        }
    })
}

struct Profile {
    code: u8,
    family: Family,
    descending: bool,
    placement: Placement,
}

enum RecordStyle {
    Raw,
    CompressedWithSlack,
}

fn finalize(
    profile: Profile,
    rows: Vec<Row>,
    final_rows: Vec<Row>,
    style: RecordStyle,
    forwarded: bool,
) -> Case {
    let Profile {
        code,
        family,
        descending,
        placement,
    } = profile;
    let raw = matches!(style, RecordStyle::Raw);
    let slack = matches!(style, RecordStyle::CompressedWithSlack);
    let records = rows
        .iter()
        .map(|row| records::original(row, family, raw, slack))
        .collect::<Vec<_>>();
    let policy = if matches!(placement, Placement::Fitted) {
        Serialization::Fitted
    } else {
        Serialization::Growing
    };
    let final_records = rows
        .iter()
        .zip(&final_rows)
        .zip(&records)
        .map(|((old, new), before)| records::final_record(before, old, new, family, policy))
        .collect::<Vec<_>>();
    let mut before = pages::original(&rows, &records, family, descending, forwarded);
    before[4..20].copy_from_slice(b"Standard ACE DB\0");
    before[20] = code;
    let (mut expected, data_pages) = match placement {
        Placement::Fitted => (
            pages::fitted(&before, &records, &final_records, forwarded),
            Vec::new(),
        ),
        Placement::Data | Placement::DataIndexes => {
            let (after, pages) = pages::growing(&before, &records, &final_records);
            (after, pages)
        }
    };
    let index_pages = pages::indexes(
        &mut expected,
        &before,
        &final_rows,
        family,
        descending,
        matches!(placement, Placement::DataIndexes),
    );
    let changed = records
        .iter()
        .zip(&final_records)
        .enumerate()
        .filter_map(|(row, (a, b))| (a != b).then_some(row))
        .collect();
    Case {
        before,
        expected,
        rows,
        final_rows,
        records,
        final_records,
        family,
        placement,
        first_rows: Vec::new(),
        second_rows: Vec::new(),
        fixed_rows: Vec::new(),
        changed,
        data_pages,
        index_pages,
        fitted_refuses: false,
    }
}

pub fn fitted(code: u8, family: Family, descending: bool, recipe: Recipe) -> Case {
    let first_low = family.low(true);
    let second_low = family.low(false);
    let first_high = family.high(true);
    let second_high = family.high(false);
    let pairs = [
        (Some(first_low.clone()), Some(second_low.clone())),
        (Some(first_high.clone()), Some(second_high.clone())),
        (None, Some(second_high.clone())),
        (Some(family.long(true)), Some(second_low.clone())),
        (None, Some(second_low)),
        (Some(first_low), Some(second_high)),
    ];
    let rows = pairs
        .into_iter()
        .enumerate()
        .map(|(ordinal, (first, second))| Row {
            id: ordinal as i32 + 1,
            age: if ordinal == 2 {
                None
            } else {
                Some(if ordinal < 2 { 7 } else { ordinal as i32 * 10 })
            },
            first,
            second,
            opaque: if ordinal == 3 {
                None
            } else {
                Some(vec![0xde, 0xad, ordinal as u8])
            },
        })
        .collect::<Vec<_>>();
    let mut final_rows = rows.clone();
    let (first_rows, second_rows, fixed_rows) = match recipe {
        Recipe::Permutation => {
            final_rows[0].first = rows[1].first.clone();
            final_rows[0].second = rows[1].second.clone();
            final_rows[1].first = rows[0].first.clone();
            final_rows[1].second = rows[0].second.clone();
            (vec![0, 1], vec![0, 1], vec![])
        }
        Recipe::NullShrinkGrow => {
            final_rows[2].first = Some(Vec::new());
            final_rows[2].second = None;
            final_rows[2].age = Some(22);
            final_rows[3].first = Some(family.low(true));
            final_rows[3].second = Some(family.long(false));
            final_rows[4].age = None;
            final_rows[5].second = Some(Vec::new());
            (vec![2, 3, 4], vec![2, 3, 4, 5], vec![2, 4])
        }
    };
    let mut case = finalize(
        Profile {
            code,
            family,
            descending,
            placement: Placement::Fitted,
        },
        rows,
        final_rows,
        RecordStyle::CompressedWithSlack,
        code == 6,
    );
    case.first_rows = first_rows;
    case.second_rows = second_rows;
    case.fixed_rows = fixed_rows;
    case
}

pub fn growing(code: u8, family: Family, index_growth: bool) -> Case {
    assert!(matches!(
        family,
        Family::LegacyBinary | Family::ModernBinary
    ));
    let rows = (0..64)
        .map(|ordinal| Row {
            id: ordinal + 1,
            age: Some(ordinal + 1),
            first: if ordinal == 0 {
                None
            } else {
                Some(vec![if ordinal % 2 == 0 { b'a' } else { b's' }])
            },
            second: Some(vec![ordinal as u8]),
            opaque: if ordinal % 5 == 3 {
                None
            } else {
                Some(vec![0xde, 0xad, ordinal as u8])
            },
        })
        .collect::<Vec<_>>();
    let mut final_rows = rows.clone();
    let units = if index_growth { 32 } else { 12 };
    let binary = if index_growth { 32 } else { 8 };
    for (ordinal, row) in final_rows.iter_mut().enumerate() {
        row.age = Some(1001 + ordinal as i32);
        row.first = Some(vec![if ordinal % 2 == 0 { b'a' } else { b's' }; units]);
        row.second = Some(vec![ordinal as u8; binary]);
    }
    let placement = if index_growth {
        Placement::DataIndexes
    } else {
        Placement::Data
    };
    let mut case = finalize(
        Profile {
            code,
            family,
            descending: false,
            placement,
        },
        rows,
        final_rows,
        RecordStyle::Raw,
        false,
    );
    case.first_rows = (0..64).collect();
    case.second_rows = (0..64).collect();
    case.fixed_rows = (0..64).collect();
    case
}

pub fn capacity(code: u8, exceeds: bool) -> Case {
    let row = Row {
        id: 1,
        age: Some(7),
        first: Some(b"a".to_vec()),
        second: Some(b"s".to_vec()),
        opaque: None,
    };
    let final_row = Row {
        first: Some(if exceeds { b"aaa".to_vec() } else { Vec::new() }),
        second: Some(if exceeds {
            b"aa".to_vec()
        } else {
            b"a".to_vec()
        }),
        ..row.clone()
    };
    let mut case = finalize(
        Profile {
            code,
            family: Family::LegacyText,
            descending: false,
            placement: if exceeds {
                Placement::Data
            } else {
                Placement::Fitted
            },
        },
        vec![row],
        vec![final_row],
        RecordStyle::Raw,
        false,
    );
    case.first_rows = vec![0];
    case.second_rows = vec![0];
    case.fitted_refuses = exceeds;
    case
}
