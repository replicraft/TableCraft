#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use std::{fs, io::Cursor};
use tablecraft_access::editing::{
    NativeEditError,
    journal::{self, NativeJournalEditor},
    offline::{Cause, OfflineWriteSession, Phase, SessionState},
};
use tablecraft_ace::{NativeImage, NativeUpdateProfile};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, LegacyDateTime, Value},
};
use tablecraft_db::{
    CellUpdate, ColumnUpdates, EditableBatch, EditableTable, SnapshotError, UpdateProfile,
};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::fixtures::access::{
    canonical_before_change as positive, canonical_fixed_before_change as positive_fixed,
    canonical_ordinary_content::{self as sources, Case, Source, Table},
    mutation_fixture,
};

const SCOPE: CatalogScope = CatalogScope::new(120);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn source(header: u8, case: Case) -> Source {
    assert!(matches!(header, 5 | 6));
    let mut source = sources::source(case);
    source.bytes[20..24].copy_from_slice(&u32::from(header).to_le_bytes());
    source
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn locked(path: &std::path::Path) {
    assert!(matches!(ExclusiveFile::open(path), Err(IoError::LockBusy)));
}

fn record(bytes: &[u8], table: &Table, ordinal: usize) -> (usize, usize) {
    let mut remaining = ordinal;
    for &page in &table.data {
        let base = page as usize * 4096;
        assert_eq!(&bytes[base + 4..base + 8], &table.definition.to_le_bytes());
        let count = usize::from(u16::from_le_bytes(
            bytes[base + 12..base + 14].try_into().unwrap(),
        ));
        if remaining >= count {
            remaining -= count;
            continue;
        }
        let word = base + 14 + 2 * remaining;
        let start = u16::from_le_bytes(bytes[word..word + 2].try_into().unwrap());
        assert_eq!(start & 0xe000, 0);
        let end = if remaining == 0 {
            4096
        } else {
            usize::from(u16::from_le_bytes(
                bytes[word - 2..word].try_into().unwrap(),
            ))
        };
        return (base + usize::from(start), base + end);
    }
    panic!("qualified literal ordinal must have a direct original record");
}

fn int32_expected(source: &Source, ordinal: usize, replacement: i32) -> Vec<u8> {
    let table = &source.tables[0];
    let Value::Int32(before) = &table.rows[ordinal][0] else {
        panic!("the selected literal field is Int32");
    };
    let (start, _) = record(&source.bytes, table, ordinal);
    assert_eq!(&source.bytes[start + 2..start + 6], &before.to_le_bytes());
    let mut expected = source.bytes.clone();
    expected[start + 2..start + 6].copy_from_slice(&replacement.to_le_bytes());
    expected
}

#[test]
fn ordinary_single_noop_and_empty_int32_requests_keep_profile_four() {
    for header in [5, 6] {
        let source = source(header, Case::Extrema);
        for mode in 0..3 {
            let editor =
                NativeJournalEditor::new(image(&source.bytes), "Items", &mut budget(), &|| false)
                    .unwrap();
            assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
            let requests = if mode == 2 {
                Vec::new()
            } else {
                vec![CellUpdate::new(
                    editor.snapshot().row_handle(0).unwrap(),
                    Value::Int32(if mode == 0 { 17 } else { 1 }),
                )]
            };
            let plan = editor
                .plan_updates("Id", &requests, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                plan.intent().profile(),
                NativeUpdateProfile::Ace16OrdinaryFixed
            );
            assert!(plan.intent().int32().is_none());
            assert_eq!(
                plan.intent().fixed().unwrap().groups()[0].updates().len(),
                usize::from(mode != 2)
            );
            assert_eq!(
                &plan.intent().encode(&mut budget(), &|| false).unwrap()[..8],
                b"TCNI0004"
            );
            let expected = if mode == 0 {
                int32_expected(&source, 0, 17)
            } else {
                source.bytes.clone()
            };
            assert_eq!(plan.after_bytes(), expected);
            assert_eq!(plan.changed_rows(), usize::from(mode == 0));
        }
    }
}

#[test]
fn ordinary_typed_and_sql_sessions_publish_twenty_eight_literal_images() {
    let cases = [
        (Case::Extrema, 0, "UPDATE [Items] SET [Id]=17 WHERE [Id]=1"),
        (
            Case::Named,
            0,
            "UPDATE [People Stats] SET [Person Id]=17 WHERE [Person Id]=7",
        ),
        (
            Case::ProgramLikeName,
            0,
            "UPDATE [AutoExec] SET [Id]=17 WHERE [Id]=1",
        ),
        (Case::XmlData, 0, "UPDATE [Items] SET [Id]=17 WHERE [Id]=7"),
        (
            Case::Multiple,
            1,
            "UPDATE [Numbers] SET [Id]=17 WHERE [Id]=2",
        ),
        (
            Case::SplitData,
            1,
            "UPDATE [Items] SET [Id]=17 WHERE [Id]=2",
        ),
        (
            Case::RelocatedMaps,
            0,
            "UPDATE [Items] SET [Id]=17 WHERE [Id]=1",
        ),
    ];
    let mut saves = 0;
    for header in [5, 6] {
        for (case, ordinal, sql) in cases {
            let source = source(header, case);
            let table = &source.tables[0];
            let expected = int32_expected(&source, ordinal, 17);
            let mut rows = table.rows.clone();
            rows[ordinal][0] = Value::Int32(17);
            for typed in [true, false] {
                let root = tempfile::tempdir().unwrap();
                let path = root.path().join("target.accdb");
                let bundle = root.path().join("journal");
                fs::write(&path, &source.bytes).unwrap();
                let session =
                    OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
                let staged = if typed {
                    let editor = session
                        .edit_table(&table.name, &mut budget(), &|| false)
                        .unwrap();
                    assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
                    assert_eq!(editor.snapshot().rows(), table.rows);
                    let handle = editor.snapshot().row_handle(ordinal).unwrap();
                    editor
                        .plan_updates(
                            &table.columns[0],
                            &[CellUpdate::new(handle, Value::Int32(17))],
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap()
                } else {
                    let query = parse_query(sql, ParseLimits::default()).unwrap();
                    session
                        .plan_update(query.query(), &mut budget(), None, &|| false)
                        .unwrap()
                };
                assert_eq!((staged.matched_rows(), staged.changed_rows()), (1, 1));
                assert_eq!(fs::read(&path).unwrap(), source.bytes);
                assert!(!bundle.exists());
                locked(&path);
                let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
                assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(session.last_completed(), Some(receipt));
                assert_eq!(fs::read(&path).unwrap(), expected);
                assert!(!bundle.exists());
                locked(&path);
                let editor = session
                    .edit_table(&table.name, &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(editor.update_profile(), UpdateProfile::StoredScalar);
                assert_eq!(editor.snapshot().rows(), rows);
                editor.into_session().release().unwrap();
                ExclusiveFile::open(&path).unwrap().close().unwrap();
                assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
                saves += 1;
            }
        }
    }
    assert_eq!(saves, 28);
}

#[test]
fn ordinary_equal_and_zero_match_sql_keep_exact_bytes_without_event_timing() {
    for header in [5, 6] {
        let source = source(header, Case::XmlData);
        for typed in [true, false] {
            for empty in [false, true] {
                let root = tempfile::tempdir().unwrap();
                let path = root.path().join("target.accdb");
                let bundle = root.path().join("journal");
                fs::write(&path, &source.bytes).unwrap();
                let session =
                    OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
                let staged = if typed {
                    let editor = session
                        .edit_table("Items", &mut budget(), &|| false)
                        .unwrap();
                    let requests = if empty {
                        Vec::new()
                    } else {
                        vec![CellUpdate::new(
                            editor.snapshot().row_handle(0).unwrap(),
                            Value::Int32(7),
                        )]
                    };
                    editor
                        .plan_updates("Id", &requests, &mut budget(), &|| false)
                        .unwrap()
                } else {
                    let sql = if empty {
                        "UPDATE Items SET Id=7 WHERE Id=999"
                    } else {
                        "UPDATE Items SET Id=7 WHERE Id=7"
                    };
                    let query = parse_query(sql, ParseLimits::default()).unwrap();
                    session
                        .plan_update(query.query(), &mut budget(), None, &|| false)
                        .unwrap()
                };
                assert_eq!(
                    (staged.matched_rows(), staged.changed_rows()),
                    (usize::from(!empty), 0)
                );
                let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
                assert_eq!(receipt.changed_rows(), Some(0));
                assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
                assert_eq!(fs::read(&path).unwrap(), source.bytes);
                assert!(!bundle.exists());
                locked(&path);
                session.release().unwrap();
                assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
            }
        }
    }
}

#[test]
fn ordinary_joint_wide_updates_preserve_masks_date_bits_and_unselected_bytes() {
    for header in [5, 6] {
        let source = source(header, Case::InactiveXml);
        let table = &source.tables[0];
        let (first, first_end) = record(&source.bytes, table, 0);
        let (second, _) = record(&source.bytes, table, 1);
        assert_eq!(&source.bytes[first + 9..first + 17], b"<Event/>");
        assert_eq!(source.bytes[first_end - 2] & 0x10, 0);
        let mut expected = source.bytes.clone();
        expected[first + 9..first + 17].copy_from_slice(&42_i64.to_le_bytes());
        expected[first_end - 2] |= 0x11;
        expected[second + 17..second + 25].copy_from_slice(&(-10_001_i64).to_le_bytes());
        expected[second + 37..second + 45].copy_from_slice(&0_u64.to_le_bytes());
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("target.accdb");
        let bundle = root.path().join("journal");
        fs::write(&path, &source.bytes).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        let first = editor.snapshot().row_handle(0).unwrap();
        let second = editor.snapshot().row_handle(1).unwrap();
        let flag = [CellUpdate::new(first, Value::Boolean(true))];
        let integer = [CellUpdate::new(first, Value::Int64(42))];
        let currency = [CellUpdate::new(
            second,
            Value::Currency(Currency::from_scaled(-10_001)),
        )];
        let date = [CellUpdate::new(
            second,
            Value::DateTime(LegacyDateTime::from_bits(0).unwrap()),
        )];
        let staged = editor
            .plan_columns(
                &[
                    ColumnUpdates {
                        column: "Flag",
                        updates: &flag,
                    },
                    ColumnUpdates {
                        column: "Int64Value",
                        updates: &integer,
                    },
                    ColumnUpdates {
                        column: "Money",
                        updates: &currency,
                    },
                    ColumnUpdates {
                        column: "DateValue",
                        updates: &date,
                    },
                ],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!((staged.matched_rows(), staged.changed_rows()), (2, 2));
        let (session, _) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(fs::read(&path).unwrap(), expected);
        assert!(!bundle.exists());
        locked(&path);
        session.release().unwrap();
    }
}

#[test]
fn ordinary_empty_batch_checks_all_sixteen_tables_and_retires_its_bundle() {
    for header in [5, 6] {
        let source = source(header, Case::Sixteen);
        for typed in [true, false] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("target.accdb");
            let bundle = root.path().join("journal");
            fs::write(&path, &source.bytes).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let staged = if typed {
                let editor = session.edit_table("T01", &mut budget(), &|| false).unwrap();
                editor
                    .plan_columns(
                        &[
                            ColumnUpdates {
                                column: "Id",
                                updates: &[],
                            },
                            ColumnUpdates {
                                column: "Score",
                                updates: &[],
                            },
                        ],
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap()
            } else {
                let query = parse_query(
                    "UPDATE T01 SET Id=17,Score=23 WHERE 1=0",
                    ParseLimits::default(),
                )
                .unwrap();
                session
                    .plan_batch_update(query.query(), &mut budget(), None, &|| false)
                    .unwrap()
            };
            assert_eq!((staged.matched_rows(), staged.changed_rows()), (0, 0));
            let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
            assert_eq!(receipt.changed_rows(), Some(0));
            assert_eq!(fs::read(&path).unwrap(), source.bytes);
            assert!(!bundle.exists());
            session.release().unwrap();
        }
    }
}

#[test]
fn ordinary_refusals_and_stale_handles_retain_bytes_lease_and_receipts() {
    for header in [5, 6] {
        for case in [
            Case::OrphanZeroPage,
            Case::UnknownHeaderByte,
            Case::MapAlias,
            Case::SharedSystemMap,
        ] {
            let source = source(header, case);
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("target.accdb");
            fs::write(&path, &source.bytes).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let editor = session
                .edit_table("Items", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(editor.update_profile(), UpdateProfile::Unavailable);
            let handle = editor.snapshot().row_handle(0).unwrap();
            let failure = editor
                .plan_updates(
                    "Id",
                    &[CellUpdate::new(handle, Value::Int32(17))],
                    &mut budget(),
                    &|| false,
                )
                .err()
                .expect("unknown ordinary content cannot fall through to another writer");
            assert_eq!(failure.phase(), Phase::Planning);
            assert_eq!(failure.pair_state(), None);
            assert_eq!(failure.current_outcome(), None);
            assert_eq!(fs::read(&path).unwrap(), source.bytes);
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
            locked(&path);
            failure.into_session().release().unwrap();
        }
        let source = source(header, Case::XmlData);
        let expected = int32_expected(&source, 0, 17);
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("target.accdb");
        let bundle = root.path().join("journal");
        fs::write(&path, &source.bytes).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        let stale = editor.snapshot().row_handle(0).unwrap();
        let staged = editor
            .plan_updates(
                "Id",
                &[CellUpdate::new(stale, Value::Int32(17))],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table("Items", &mut budget(), &|| false)
            .unwrap();
        let failure = editor
            .plan_updates(
                "Id",
                &[CellUpdate::new(stale, Value::Int32(19))],
                &mut budget(),
                &|| false,
            )
            .err()
            .expect("the new snapshot cannot accept a prior handle");
        assert!(matches!(
            failure.cause(),
            Cause::Editing(journal::Error::Native(NativeEditError::Snapshot(
                SnapshotError::StaleRow
            )))
        ));
        assert_eq!(failure.state(), SessionState::Ready);
        assert_eq!(failure.last_completed(), Some(receipt));
        assert_eq!(failure.current_outcome(), None);
        assert_eq!(fs::read(&path).unwrap(), expected);
        assert!(!bundle.exists());
        locked(&path);
        failure.into_session().release().unwrap();
    }
}

#[test]
fn existing_positive_and_jet4_intent_profiles_remain_distinct() {
    for header in [5, 6] {
        let positive = positive::source(header, positive::Recipe::Sequential);
        let editor =
            NativeJournalEditor::new(image(&positive.before), "Items", &mut budget(), &|| false)
                .unwrap();
        let row = editor.snapshot().row_handle(0).unwrap();
        let plan = editor
            .plan_updates(
                "Score",
                &[CellUpdate::new(row, Value::Int32(17))],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(
            plan.intent().profile(),
            NativeUpdateProfile::Ace16BeforeChangeInt32
        );
        assert_eq!(
            &plan.intent().encode(&mut budget(), &|| false).unwrap()[..8],
            b"TCNI0002"
        );
        assert_eq!(plan.after_bytes(), positive.expected.as_ref().unwrap());
        let positive = positive_fixed::source(header, positive_fixed::Recipe::FlagChange);
        let editor =
            NativeJournalEditor::new(image(&positive.before), "Items", &mut budget(), &|| false)
                .unwrap();
        let row = editor.snapshot().row_handle(0).unwrap();
        let plan = editor
            .plan_updates(
                "Flag",
                &[CellUpdate::new(row, Value::Boolean(true))],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(
            plan.intent().profile(),
            NativeUpdateProfile::Ace16BeforeChangeFixed
        );
        assert_eq!(
            &plan.intent().encode(&mut budget(), &|| false).unwrap()[..8],
            b"TCNI0003"
        );
        assert_eq!(plan.after_bytes(), positive.expected.as_ref().unwrap());
    }
    let before = mutation_fixture::database();
    let editor =
        NativeJournalEditor::new(image(&before), "Items", &mut budget(), &|| false).unwrap();
    let row = editor.snapshot().row_handle(0).unwrap();
    let plan = editor
        .plan_updates(
            "Score",
            &[CellUpdate::new(row, Value::Int32(12))],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(plan.intent().profile(), NativeUpdateProfile::Jet4Int32);
    assert_eq!(
        &plan.intent().encode(&mut budget(), &|| false).unwrap()[..8],
        b"TCNI0001"
    );
    let at = 12 * 4096 + 4091;
    assert_eq!(&before[at..at + 4], &10_i32.to_le_bytes());
    let mut expected = before;
    expected[at..at + 4].copy_from_slice(&12_i32.to_le_bytes());
    assert_eq!(plan.after_bytes(), expected);
}
