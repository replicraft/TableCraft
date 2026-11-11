//! Real reviewed SQL ADD connects absence semantics to existing row writers.
use super::{inspect, review, same};
use crate::tests::{request, stored::opened, terminal};
use crate::{
    Command, EditInput, FieldAdditionOrigin, InsertInput, InsertTarget, ParameterInput, Response,
    SaveKnownOutcome, SaveOutcome, SaveWorker, Worker,
};
use std::{fs, path::Path};
use tablecraft_core::values::{Currency, Decimal, Double, Guid, LegacyDateTime, Single, Value};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    declared_fields, deletes,
    field_changes::inserting,
    fields,
    typed_fields::Kind,
    unadorned_fields::{self as raw, rows},
};

struct Run<'a> {
    worker: &'a Worker,
    path: &'a Path,
    saves: SaveWorker,
    revision: u64,
    expected: Vec<u8>,
}

impl Run<'_> {
    fn finish(&mut self, id: u64, after: Vec<u8>) {
        let report = terminal(&self.saves);
        assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
                if receipt.known == SaveKnownOutcome::Committed));
        assert!(self.saves.acknowledge(id));
        same(&fs::read(self.path).unwrap(), &after);
        assert!(!crate::journal_path(self.path).exists());
        self.expected = after;
    }

    fn reopened(&mut self, command: Command) {
        let Response::Opened {
            document_revision, ..
        } = request(self.worker, command).unwrap()
        else {
            panic!("actual fresh open");
        };
        assert!(document_revision > self.revision);
        self.revision = document_revision;
    }

    fn add(&mut self, sql: &str, after: Vec<u8>) {
        let target = inspect(self.worker, self.revision, sql);
        let Response::FieldAdditionReviewed {
            review: facts,
            capability,
        } = review(self.worker, &target).unwrap()
        else {
            panic!("actual unadorned declaration review");
        };
        assert!(matches!(facts.origin, FieldAdditionOrigin::SqlUnadorned(_)));
        assert_eq!(facts.origin, capability.captured_origin());
        let witness = capability.reopen_witness();
        same(witness.expected_bytes(), &after);
        same(&fs::read(self.path).unwrap(), &self.expected);
        let id = self
            .saves
            .start_field_addition(capability, crate::journal_path(self.path))
            .unwrap();
        self.finish(id, after);
        self.reopened(Command::OpenColumnAdded { witness });
    }

    fn update(&mut self, ddl: &str, kind: &str, text: &str, is_null: bool, after: Vec<u8>) {
        let sql = format!(
            "PARAMETERS replacement {ddl}; UPDATE Items SET AddedValue=replacement WHERE Score=23"
        );
        let target = inspect(self.worker, self.revision, &sql);
        let Response::QueryUpdateReviewed { review, capability } = request(
            self.worker,
            Command::ReviewQueryUpdate {
                target,
                parameters: vec![ParameterInput {
                    name: "replacement".into(),
                    kind: kind.into(),
                    text: text.into(),
                    is_null,
                }],
            },
        )
        .unwrap_or_else(|error| panic!("actual {ddl} update refused: {error}")) else {
            panic!("typed action review");
        };
        assert_eq!((review.matched_rows, review.changed_rows), (1, 1));
        same(&fs::read(self.path).unwrap(), &self.expected);
        let id = self
            .saves
            .start_query_update(capability, crate::journal_path(self.path))
            .unwrap();
        self.finish(id, after);
        self.reopened(crate::tests::open(self.path, ""));
    }

    fn insert(&mut self, table: &str, inputs: Vec<InsertInput>, after: Vec<u8>) {
        let Response::InsertDraft(draft) = request(
            self.worker,
            Command::BeginInsert {
                target: InsertTarget {
                    document_revision: self.revision,
                    table: table.into(),
                },
            },
        )
        .unwrap() else {
            panic!("actual insert draft");
        };
        let Response::InsertReviewed { review, capability } = request(
            self.worker,
            Command::ReviewInsert {
                draft_id: draft.id,
                inputs,
            },
        )
        .unwrap_or_else(|error| panic!("fresh unadorned INSERT refused: {error}")) else {
            panic!("actual consuming INSERT");
        };
        assert_eq!(review.inserted_rows, 1);
        same(&fs::read(self.path).unwrap(), &self.expected);
        let id = self
            .saves
            .start_insert(capability, crate::journal_path(self.path))
            .unwrap();
        self.finish(id, after);
        self.reopened(crate::tests::open(self.path, ""));
    }
}

struct ScalarCase {
    ddl: &'static str,
    token: &'static str,
    text: &'static str,
    kind: Kind,
    value: Value,
}

fn cases() -> [ScalarCase; 12] {
    [
        ScalarCase {
            ddl: "BIT",
            token: "boolean",
            text: "true",
            kind: Kind::Boolean,
            value: Value::Boolean(true),
        },
        ScalarCase {
            ddl: "BYTE",
            token: "byte",
            text: "200",
            kind: Kind::Byte,
            value: Value::Byte(200),
        },
        ScalarCase {
            ddl: "SHORT",
            token: "int16",
            text: "-2345",
            kind: Kind::Int16,
            value: Value::Int16(-2345),
        },
        ScalarCase {
            ddl: "LONG",
            token: "int32",
            text: "124",
            kind: Kind::Int32,
            value: Value::Int32(124),
        },
        ScalarCase {
            ddl: "BIGINT",
            token: "int64",
            text: "9007199254740993",
            kind: Kind::Int64,
            value: Value::Int64(9_007_199_254_740_993),
        },
        ScalarCase {
            ddl: "SINGLE",
            token: "single",
            text: "1.25",
            kind: Kind::Single,
            value: Value::Single(Single::from_bits(0x3fa0_0000)),
        },
        ScalarCase {
            ddl: "DOUBLE",
            token: "double",
            text: "1.23456789012345",
            kind: Kind::Double,
            value: Value::Double(Double::from_bits(0x3ff3_c0ca_428c_59dd)),
        },
        ScalarCase {
            ddl: "CURRENCY",
            token: "currency",
            text: "5.25",
            kind: Kind::Currency,
            value: Value::Currency(Currency::from_scaled(52_500)),
        },
        ScalarCase {
            ddl: "DATETIME",
            token: "date",
            text: "1900-01-01T12:00:00",
            kind: Kind::DateTime,
            value: Value::DateTime(LegacyDateTime::from_bits(0x4004_0000_0000_0000).unwrap()),
        },
        ScalarCase {
            ddl: "GUID",
            token: "guid",
            text: "00112233-4455-6677-8899-aabbccddeeff",
            kind: Kind::Guid,
            value: Value::Guid(Guid::from_bytes([
                0, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255,
            ])),
        },
        ScalarCase {
            ddl: "DECIMAL(28,4)",
            token: "decimal",
            text: "9007199254740993.1234",
            kind: Kind::Decimal {
                precision: 28,
                scale: 4,
            },
            value: Value::Decimal(Decimal::new(90_071_992_547_409_931_234, 4).unwrap()),
        },
        ScalarCase {
            ddl: "TEXT(80)",
            token: "text",
            text: "雪🙂",
            kind: Kind::ShortText {
                units: 80,
                compression: false,
            },
            value: Value::Text("雪🙂".into()),
        },
    ]
}

#[test]
fn every_unadorned_scalar_add_connects_historical_values_exact_update_and_fresh_insert() {
    for case in cases() {
        let mut source = declared_fields::source();
        source.bytes = inserting::keyless_score(&source.bytes, source.definition, None);
        let (_directory, path, worker, revision) = opened(&source.bytes);
        let mut run = Run {
            worker: &worker,
            path: &path,
            saves: SaveWorker::new(|| {}),
            revision,
            expected: source.bytes,
        };
        let added = raw::after(
            &run.expected,
            source.definition,
            "AddedValue",
            case.kind,
            [9, 4, 0, 1],
        );
        run.add(
            &format!("ALTER TABLE Items ADD COLUMN AddedValue {}", case.ddl),
            added,
        );
        let Response::Data {
            rows: historical, ..
        } = request(
            &worker,
            Command::Page {
                table: "Items".into(),
                offset: 0,
            },
        )
        .unwrap()
        else {
            panic!("historical rows");
        };
        assert_eq!(historical.len(), 2);
        if matches!(case.kind, Kind::Boolean) {
            assert!(historical.iter().all(|row| matches!(
                row[1].kind,
                crate::CellKind::Scalar {
                    boolean: Some(false),
                    ..
                }
            )));
        } else {
            assert!(
                historical.iter().all(|row| matches!(
                    row[1].kind,
                    crate::CellKind::Scalar { is_null: true, .. }
                ))
            );
        }
        let updated = rows::update(&run.expected, source.definition, 0, case.kind, &case.value);
        run.update(case.ddl, case.token, case.text, false, updated);
        let cleared_value = if matches!(case.kind, Kind::Boolean) {
            Value::Boolean(false)
        } else {
            Value::Null
        };
        let cleared = rows::update(
            &run.expected,
            source.definition,
            0,
            case.kind,
            &cleared_value,
        );
        run.update(
            case.ddl,
            case.token,
            if matches!(case.kind, Kind::Boolean) {
                "false"
            } else {
                ""
            },
            !matches!(case.kind, Kind::Boolean),
            cleared,
        );
        if matches!(case.kind, Kind::Boolean) {
            let Response::InsertDraft(draft) = request(
                &worker,
                Command::BeginInsert {
                    target: InsertTarget {
                        document_revision: run.revision,
                        table: "Items".into(),
                    },
                },
            )
            .unwrap() else {
                panic!("actual BOOLEAN omission draft");
            };
            assert!(
                request(
                    &worker,
                    Command::ReviewInsert {
                        draft_id: draft.id,
                        inputs: vec![InsertInput::Omitted, InsertInput::Omitted]
                    }
                )
                .is_err(),
                "no implicit Boolean default invented"
            );
            same(&fs::read(&path).unwrap(), &run.expected);
        }
        if matches!(case.kind, Kind::ShortText { .. }) {
            let target = inspect(
                &worker,
                run.revision,
                "PARAMETERS replacement TEXT(80); UPDATE Items SET AddedValue=replacement WHERE Score=23",
            );
            assert!(
                request(
                    &worker,
                    Command::ReviewQueryUpdate {
                        target,
                        parameters: vec![ParameterInput {
                            name: "replacement".into(),
                            kind: "text".into(),
                            text: String::new(),
                            is_null: false
                        }]
                    }
                )
                .is_err(),
                "absent AZL must not invent permission for empty text"
            );
            same(&fs::read(&path).unwrap(), &run.expected);
        }
        let fresh = if matches!(case.kind, Kind::Boolean) {
            Value::Boolean(false)
        } else {
            Value::Null
        };
        let after =
            rows::append_resolved(&run.expected, source.definition, case.kind, None, &fresh);
        let input = if matches!(case.kind, Kind::Boolean) {
            InsertInput::Value(EditInput {
                text: "false".into(),
                is_null: false,
            })
        } else {
            InsertInput::Omitted
        };
        run.insert("Items", vec![InsertInput::Omitted, input], after);
        let Response::Data { rows: actual, .. } = request(
            &worker,
            Command::Page {
                table: "Items".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("fresh actual values");
        };
        assert_eq!(actual.len(), 3);
        if !matches!(case.kind, Kind::Boolean) {
            assert!(matches!(
                actual[2][1].kind,
                crate::CellKind::Scalar { is_null: true, .. }
            ));
        }
    }
}

#[test]
fn generated_history_add_update_and_omitted_insert_use_actual_last_and_no_new_default() {
    let source = fields::source(fields::Case::FirstNumber);
    let before = deletes::after(
        &source.bytes,
        source.definition,
        &[*source.roots.last().unwrap()],
    );
    let at = source.definition as usize * 4096 + 20;
    let last = i32::from_le_bytes(before[at..at + 4].try_into().unwrap());
    assert!(last > 1);
    let (_directory, path, worker, revision) = opened(&before);
    let mut run = Run {
        worker: &worker,
        path: &path,
        saves: SaveWorker::new(|| {}),
        revision,
        expected: before,
    };
    let added = raw::after(
        &run.expected,
        source.definition,
        "Salary",
        Kind::Currency,
        [0; 4],
    );
    run.add("ALTER TABLE Table1 ADD COLUMN Salary CURRENCY", added);
    let target = inspect(
        &worker,
        run.revision,
        "UPDATE Table1 SET Salary=CCUR(5.25) WHERE ID=1",
    );
    let Response::QueryUpdateReviewed { review, capability } = request(
        &worker,
        Command::ReviewQueryUpdate {
            target,
            parameters: Vec::new(),
        },
    )
    .unwrap() else {
        panic!("generated companion update");
    };
    assert_eq!((review.matched_rows, review.changed_rows), (1, 1));
    let updated = rows::update(
        &run.expected,
        source.definition,
        source.roots[0],
        Kind::Currency,
        &Value::Currency(Currency::from_scaled(52_500)),
    );
    let id = run
        .saves
        .start_query_update(capability, crate::journal_path(&path))
        .unwrap();
    run.finish(id, updated);
    run.reopened(crate::tests::open(&path, ""));
    assert_eq!(
        i32::from_le_bytes(run.expected[at..at + 4].try_into().unwrap()),
        last
    );
    let inserted = raw::append_currency(&run.expected, source.definition, Some(0), None);
    run.insert(
        "Table1",
        vec![
            InsertInput::Omitted,
            InsertInput::Omitted,
            InsertInput::Omitted,
        ],
        inserted,
    );
    assert_eq!(
        i32::from_le_bytes(run.expected[at..at + 4].try_into().unwrap()),
        last + 1
    );
    let Response::Data { rows: actual, .. } = request(
        &worker,
        Command::Page {
            table: "Table1".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("generated fresh rows");
    };
    assert_eq!(actual.len(), 2);
    assert_eq!(actual[1][0].text, (last + 1).to_string());
    assert!(matches!(
        actual[1][2].kind,
        crate::CellKind::Scalar { is_null: true, .. }
    ));
}
