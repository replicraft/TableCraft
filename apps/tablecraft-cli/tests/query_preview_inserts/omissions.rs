//! Original-source default declarations plus the actual literal INSERT image owner.
use super::{PAGE, budget, checked, expected, fixture, fs, run, source, support};
use std::io::Cursor;
use tablecraft_access::engineering::{ScalarInsertMode, sql_insert};
use tablecraft_ace::{NativeImage, read_table};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};

struct Case {
    before: Vec<u8>,
    wanted: Vec<u8>,
    sql: &'static str,
    final_row: Vec<Value>,
    generated: bool,
}

fn with_default(code: u8, expression: &str) -> Vec<u8> {
    let mut before = source(code, false);
    let properties = fixture::with_rule("DefaultValue", expression);
    before[4 * PAGE..5 * PAGE].copy_from_slice(&properties[4 * PAGE..5 * PAGE]);
    before
}

fn cases(code: u8) -> Vec<Case> {
    let declared = with_default(code, "40");
    let missing_id = source(code, false);
    let mut null_id_expected = expected(&missing_id, false);
    // Existing expected owner independently writes the new fourth11B record.
    // This separate case omits unindexed nullable Id: zero its fresh payload and
    // clear only its presence bit. The required Score key remains exactly40.
    let start = usize::from(u16::from_le_bytes(
        null_id_expected[12 * PAGE + 20..12 * PAGE + 22]
            .try_into()
            .unwrap(),
    ));
    assert_eq!(
        &null_id_expected[12 * PAGE + start..12 * PAGE + start + 2],
        &[2, 0]
    );
    assert_eq!(
        &null_id_expected[12 * PAGE + start + 2..12 * PAGE + start + 6],
        &4_i32.to_le_bytes()
    );
    null_id_expected[12 * PAGE + start + 2..12 * PAGE + start + 6].fill(0);
    assert_eq!(null_id_expected[12 * PAGE + start + 10], 3);
    null_id_expected[12 * PAGE + start + 10] = 2;

    let unknown = with_default(code, "Date()");
    let mut result = vec![
        Case {
            wanted: expected(&declared, false),
            before: declared,
            sql: "INSERT INTO Items(Id) VALUES(4)",
            final_row: vec![Value::Int32(4), Value::Int32(40)],
            generated: false,
        },
        Case {
            wanted: null_id_expected,
            before: missing_id,
            sql: "INSERT INTO Items(Score) VALUES(40)",
            final_row: vec![Value::Null, Value::Int32(40)],
            generated: false,
        },
        Case {
            wanted: expected(&unknown, false),
            before: unknown,
            sql: "INSERT INTO Items(Score,Id) VALUES(40,4)",
            final_row: vec![Value::Int32(4), Value::Int32(40)],
            generated: false,
        },
    ];
    result.push(generated_case(code));
    result
}

pub(super) fn three_field_source(code: u8, generated: bool) -> Vec<u8> {
    let mut before = source(code, generated);
    let original_definition = before[10 * PAGE..11 * PAGE].to_vec();
    let fields = fixture::definition(
        &[("Id", 4, 0, 4), ("Score", 4, 4, 4), ("Other", 4, 8, 4)],
        3,
        11 << 8,
    );
    let mut definition = vec![0; PAGE];
    definition[..75].copy_from_slice(&original_definition[..75]);
    // Maximum native slot count, variable field count and live declarations
    // must describe the same genuine three-fixed-field source layout.
    support::put16(&mut definition, 41, 3);
    assert_eq!(&definition[43..45], &[0, 0]);
    support::put16(&mut definition, 45, 3);
    definition[75..180].copy_from_slice(&fields[63..168]);
    definition[75 + 25 + 15] = if generated { 7 } else { 3 };
    // Retain the actual original physical/logical primary and key name. Only
    // declaration positioning changes to accommodate the third fixed field.
    assert_eq!(&original_definition[241..243], &[255, 255]);
    definition[180..280].copy_from_slice(&original_definition[143..243]);
    support::put32(&mut definition, 8, 280);
    support::put16(&mut definition, 2, (PAGE - 280 - 8) as u16);
    before[10 * PAGE..11 * PAGE].copy_from_slice(&definition);

    set_other_default(&mut before, "99");
    let source_rows = [[1, 10, 100], [2, 20, 200], [3, 30, 300]];
    before[12 * PAGE..13 * PAGE].copy_from_slice(&three_field_page(&source_rows));
    before
}

pub(super) fn set_other_default(before: &mut [u8], expression: &str) {
    let mut properties = fixture::with_rule("DefaultValue", expression);
    let score = [83, 0, 99, 0, 111, 0, 114, 0, 101, 0];
    let other = [79, 0, 116, 0, 104, 0, 101, 0, 114, 0];
    let hits = properties[4 * PAGE..5 * PAGE]
        .windows(score.len())
        .enumerate()
        .filter_map(|(at, bytes)| (bytes == score).then_some(at))
        .collect::<Vec<_>>();
    assert_eq!(hits.len(), 1);
    let at = 4 * PAGE + hits[0];
    properties[at..at + other.len()].copy_from_slice(&other);
    before[4 * PAGE..5 * PAGE].copy_from_slice(&properties[4 * PAGE..5 * PAGE]);
}

fn generated_case(code: u8) -> Case {
    let before = three_field_source(code, true);
    let source_rows = [[1, 10, 100], [2, 20, 200], [3, 30, 300]];
    let mut wanted = expected(&before, true);
    wanted[12 * PAGE..13 * PAGE].copy_from_slice(&three_field_page(&[
        source_rows[0],
        source_rows[1],
        source_rows[2],
        [4, 31, 99],
    ]));
    Case {
        before,
        wanted,
        sql: "INSERT INTO Items(Id) VALUES(4)",
        final_row: vec![Value::Int32(4), Value::Int32(31), Value::Int32(99)],
        generated: true,
    }
}

fn three_field_page(rows: &[[i32; 3]]) -> Vec<u8> {
    let records = rows
        .iter()
        .map(|values| {
            let mut row = 3_u16.to_le_bytes().to_vec();
            for value in values {
                row.extend(value.to_le_bytes());
            }
            row.push(7);
            assert_eq!(row.len(), 15);
            (0, row)
        })
        .collect::<Vec<_>>();
    let mut page = support::data_page(10, &records);
    support::put16(&mut page, 2, (PAGE - 14 - rows.len() * 17) as u16);
    page
}

fn qualify_generated(before: &[u8], wanted: &[u8]) {
    let schema = tablecraft_ace::inspect_table_schema(
        &mut Cursor::new(before),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        schema
            .columns()
            .iter()
            .map(|column| (column.name(), column.number(), column.flags()))
            .collect::<Vec<_>>(),
        [("Id", 0, 3), ("Score", 1, 7), ("Other", 2, 3)]
    );
    assert_eq!(
        &before[10 * PAGE + 20..10 * PAGE + 24],
        &30_i32.to_le_bytes()
    );
    assert_eq!(
        &wanted[10 * PAGE + 20..10 * PAGE + 24],
        &31_i32.to_le_bytes()
    );
    let properties = tablecraft_ace::inspect_table_properties(
        &mut Cursor::new(before),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let other = properties
        .groups()
        .iter()
        .find(|group| group.name() == "Other")
        .unwrap();
    assert_eq!(other.get("DefaultValue").unwrap().text().unwrap(), "99");
    let index = tablecraft_ace::inspect_table_index_counter(
        &mut Cursor::new(wanted),
        CatalogScope::new(1),
        "Items",
        "ScoreKey",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(index.value(), 4);
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn qualify_omission_frontend_sources_and_complete_expected_before_action() {
    for code in [5, 6] {
        for case in cases(code) {
            checked(&case.before, 3);
            checked(&case.wanted, 4);
            if case.generated {
                qualify_generated(&case.before, &case.wanted);
            }
            let table = read_table(
                &mut Cursor::new(&case.wanted),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(table.rows()[3], case.final_row);
            assert_eq!(case.before.len(), case.wanted.len());
            for page in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 14] {
                assert_eq!(
                    &case.before[page * PAGE..(page + 1) * PAGE],
                    &case.wanted[page * PAGE..(page + 1) * PAGE]
                );
            }
        }
        for expression in ["Date()", "20", "40"] {
            checked(&with_default(code, expression), 3);
        }
    }
}

#[test]
fn access_omission_service_matches_whole_images_and_complete_service_stays_strict() {
    for code in [5, 6] {
        for case in cases(code) {
            let query = parse_query(case.sql, ParseLimits::default()).unwrap();
            let copy = sql_insert::preview_insert_with_omissions(
                capture(&case.before),
                query.query(),
                if case.generated {
                    ScalarInsertMode::Incrementing
                } else {
                    ScalarInsertMode::Explicit
                },
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(copy.before_bytes(), case.before);
            assert_eq!(copy.candidate_bytes(), case.wanted);
            if case.sql == "INSERT INTO Items(Id) VALUES(4)" {
                assert!(
                    sql_insert::preview_insert(
                        capture(&case.before),
                        query.query(),
                        if case.generated {
                            ScalarInsertMode::Incrementing
                        } else {
                            ScalarInsertMode::Explicit
                        },
                        &mut budget(),
                        &|| false
                    )
                    .is_err()
                );
            }
        }
    }
}

#[test]
fn cli_named_omissions_publish_exact_copies_and_preserve_original_sources() {
    for code in [5, 6] {
        for case in cases(code) {
            let directory = tempfile::tempdir().unwrap();
            let input = directory.path().join("before.accdb");
            let output = directory.path().join("after.accdb");
            fs::write(&input, &case.before).unwrap();
            let result = run(case.sql, &input, &output, case.generated);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(fs::read(&output).unwrap(), case.wanted);
            assert_eq!(fs::read(&input).unwrap(), case.before);
        }
    }
}

#[test]
fn needed_unknown_default_explicit_null_required_and_duplicate_key_publish_nothing() {
    for code in [5, 6] {
        for (expression, sql) in [
            ("Date()", "INSERT INTO Items(Id) VALUES(4)"),
            ("40", "INSERT INTO Items(Id,Score) VALUES(4,NULL)"),
            ("20", "INSERT INTO Items(Id) VALUES(4)"),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let before = with_default(code, expression);
            let input = directory.path().join("before.accdb");
            let output = directory.path().join("after.accdb");
            fs::write(&input, &before).unwrap();
            let result = run(sql, &input, &output, false);
            assert_eq!(result.status.code(), Some(2));
            assert!(result.stdout.is_empty());
            assert!(!output.exists());
            assert_eq!(fs::read(input).unwrap(), before);
        }
    }
}
