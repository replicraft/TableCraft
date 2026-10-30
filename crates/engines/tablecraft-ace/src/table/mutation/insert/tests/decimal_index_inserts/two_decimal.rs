//! Two independently declared NUMERIC domains coexist in one physical key.
use super::{model, original};
use model::{PAGE, Settings, numeric_fixture};
use std::{collections::BTreeSet, io::Cursor};
use tablecraft_core::values::{Decimal, Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;
use tablecraft_testkit::access::multiple_indexes as frames;

#[allow(dead_code)]
mod fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/decimal_composite_indexes/fixture.rs"
    ));
}

#[derive(Clone)]
struct Row {
    id: i32,
    first: Option<i128>,
    second: Option<i128>,
}

impl Row {
    fn values(&self) -> Vec<Value> {
        vec![
            Value::Int32(self.id),
            self.first.map_or(Value::Null, |coefficient| {
                Value::Decimal(Decimal::new(coefficient, 4).unwrap())
            }),
            self.second.map_or(Value::Null, |coefficient| {
                Value::Decimal(Decimal::new(coefficient, 2).unwrap())
            }),
        ]
    }
}

fn literal(row: &Row, code: u8, order: &[(usize, bool)]) -> (Vec<u8>, Vec<u8>) {
    use fixture::Cell;
    let values = vec![
        Some(Cell::Number(i64::from(row.id))),
        row.first.map(Cell::Decimal),
        row.second.map(Cell::Decimal),
    ];
    let bytes = fixture::with_parameters(
        code,
        &[4, 16, 16],
        &[(0, 0), (28, 4), (3, 2)],
        order,
        &[values],
        0,
    );
    let data = &bytes[12 * PAGE..13 * PAGE];
    let start = u16::from_le_bytes(data[14..16].try_into().unwrap()) as usize;
    let length = [row.first, row.second]
        .into_iter()
        .map(|value| if value.is_some() { 18 } else { 1 })
        .sum::<usize>();
    (
        data[start..].to_vec(),
        bytes[14 * PAGE + 480..14 * PAGE + 480 + length].to_vec(),
    )
}

fn keys(
    rows: &[(Row, u32)],
    code: u8,
    order: &[(usize, bool)],
    secondary: bool,
    ignored: bool,
) -> Vec<Vec<u8>> {
    let mut keys = Vec::new();
    for (row, reference) in rows {
        let mut key = if secondary {
            if ignored && row.first.is_none() && row.second.is_none() {
                continue;
            }
            literal(row, code, order).1
        } else {
            [
                vec![127],
                ((row.id as u32) ^ 0x8000_0000).to_be_bytes().to_vec(),
            ]
            .concat()
        };
        key.extend(reference.to_be_bytes());
        keys.push(key);
    }
    keys.sort();
    keys
}

fn typed(bytes: &[u8], rows: &[(Row, u32)]) {
    let snapshot = crate::read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(snapshot.rows().len(), rows.len());
    for (actual, (row, _)) in snapshot.rows().iter().zip(rows) {
        assert_eq!(actual, &row.values());
    }
}

#[test]
fn different_decimal_precision_and_scale_domains_bind_each_written_component() {
    for code in [5, 6] {
        for descending in [false, true] {
            for reversed in [false, true] {
                for ignored in [false, true] {
                    let mut settings = Settings::ordinary(true, descending);
                    settings.reversed = reversed;
                    settings.ignored = ignored;
                    let mut before = model::source(code, settings, false).before;
                    let order = if reversed {
                        vec![(2, !descending), (1, descending)]
                    } else {
                        vec![(1, descending), (2, !descending)]
                    };
                    let field = 10 * PAGE + 87 + 50;
                    before[field] = 16;
                    before[field + 11] = 3;
                    before[field + 12] = 2;
                    before[field + 15] = 2;
                    support::put16(&mut before, field + 7, 1);
                    support::put16(&mut before, field + 21, 0);
                    support::put16(&mut before, field + 23, 17);
                    support::put16(&mut before, 10 * PAGE + 43, 2);
                    let maximum = 10_i128.pow(28) - 1;
                    let source = [
                        Row {
                            id: 1,
                            first: Some(-maximum),
                            second: Some(999),
                        },
                        Row {
                            id: 2,
                            first: Some(0),
                            second: Some(-999),
                        },
                        Row {
                            id: 3,
                            first: None,
                            second: None,
                        },
                    ];
                    let mut encoded = source
                        .iter()
                        .map(|row| (0, literal(row, code, &order).0))
                        .collect::<Vec<_>>();
                    // Existing masked-NULL variable FIELD capacity is opaque;
                    // only new NULL rows have the canonical zero-byte payload.
                    encoded[2].1 = [
                        3_u16.to_le_bytes().to_vec(),
                        3_i32.to_le_bytes().to_vec(),
                        vec![0xa5; 17],
                        vec![23, 0, 23, 0, 6, 0, 2, 0, 1],
                    ]
                    .concat();
                    let mut data = support::data_page(10, &encoded);
                    let free = PAGE
                        - 14
                        - 2 * encoded.len()
                        - encoded.iter().map(|(_, body)| body.len()).sum::<usize>();
                    support::put16(&mut data, 2, free as u16);
                    frames::install(&mut before, 12, &data);
                    let mut rows = source
                        .into_iter()
                        .enumerate()
                        .map(|(slot, row)| (row, (12 << 8) | slot as u32))
                        .collect::<Vec<_>>();
                    for physical in 0..2 {
                        let stream = keys(&rows, code, &order, physical == 1, ignored);
                        frames::install(
                            &mut before,
                            13 + physical,
                            &frames::leaf(None, &stream, 0, 0, 0),
                        );
                    }
                    original::checked(&before, rows.len(), 4);
                    typed(&before, &rows);
                    let added = [
                        Row {
                            id: 10,
                            first: Some(maximum),
                            second: Some(1),
                        },
                        Row {
                            id: 11,
                            first: Some(1),
                            second: Some(999),
                        },
                        Row {
                            id: 12,
                            first: None,
                            second: Some(1),
                        },
                        Row {
                            id: 13,
                            first: None,
                            second: None,
                        },
                    ];
                    let mut expected = before.clone();
                    let old_keys = keys(&rows, code, &order, true, ignored);
                    for row in &added {
                        let count = u16::from_le_bytes(
                            expected[12 * PAGE + 12..12 * PAGE + 14].try_into().unwrap(),
                        ) as usize;
                        let end = u16::from_le_bytes(
                            expected[12 * PAGE + 12 + count * 2..12 * PAGE + 14 + count * 2]
                                .try_into()
                                .unwrap(),
                        ) as usize;
                        let body = literal(row, code, &order).0;
                        let start = end - body.len();
                        expected[12 * PAGE + start..12 * PAGE + end].copy_from_slice(&body);
                        support::put16(&mut expected, 12 * PAGE + 14 + count * 2, start as u16);
                        support::put16(&mut expected, 12 * PAGE + 12, (count + 1) as u16);
                        support::put16(
                            &mut expected,
                            12 * PAGE + 2,
                            (start - 14 - 2 * (count + 1)) as u16,
                        );
                        rows.push((row.clone(), (12 << 8) | count as u32));
                    }
                    let after_keys = keys(&rows, code, &order, true, ignored);
                    let families = |keys: &[Vec<u8>]| {
                        keys.iter()
                            .map(|key| key[..key.len() - 4].to_vec())
                            .collect::<BTreeSet<_>>()
                    };
                    let delta = families(&after_keys)
                        .difference(&families(&old_keys))
                        .count();
                    for physical in 0..2 {
                        let stream = keys(&rows, code, &order, physical == 1, ignored);
                        let page = (13 + physical) as usize;
                        frames::install(
                            &mut expected,
                            page as u32,
                            &frames::leaf(
                                Some(&before[page * PAGE..(page + 1) * PAGE]),
                                &stream,
                                0,
                                0,
                                0,
                            ),
                        );
                    }
                    support::put32(&mut expected, 10 * PAGE + 16, rows.len() as u32);
                    support::put32(&mut expected, 10 * PAGE + 67, 341);
                    support::put32(&mut expected, 10 * PAGE + 79, 659 + delta as u32);
                    original::checked(&expected, rows.len(), 4);
                    typed(&expected, &rows);
                    let requests = added
                        .iter()
                        .map(|row| crate::RowInsert::new(row.values()))
                        .collect::<Vec<_>>();
                    let candidate = original::capture(&before)
                        .preview_ace16_insert_rows(
                            "Items",
                            &requests,
                            &mut original::budget(),
                            &|| false,
                        )
                        .unwrap();
                    let actual = candidate.candidate_bytes();
                    assert_eq!(actual.len(), expected.len());
                    let difference = actual
                        .iter()
                        .zip(&expected)
                        .position(|(actual, expected)| actual != expected);
                    assert!(
                        difference.is_none(),
                        "first two-domain Decimal image difference at {difference:?}"
                    );
                    typed(actual, &rows);
                    // A valid value for one Decimal domain is not valid for its
                    // neighbor: scales are never borrowed from the first field.
                    for (first_scale, second_scale) in [(3, 2), (4, 4)] {
                        let bad = crate::RowInsert::new(vec![
                            Value::Int32(20),
                            Value::Decimal(Decimal::new(1, first_scale).unwrap()),
                            Value::Decimal(Decimal::new(1, second_scale).unwrap()),
                        ]);
                        assert!(
                            original::capture(&before)
                                .preview_ace16_insert_rows(
                                    "Items",
                                    &[bad],
                                    &mut original::budget(),
                                    &|| false
                                )
                                .is_err()
                        );
                    }
                }
            }
        }
    }
}
