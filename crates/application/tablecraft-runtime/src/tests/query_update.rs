//! SQL evaluation ends at review; the save consumes one source-bound native intent.
use super::{open, query_parameters::inspect, request, stored::opened, terminal};
use crate::{
    Command, Error, Response, ReviewedQueryUpdate, SaveFailureKind, SaveOutcome, SaveStartFailure,
    SaveWorker,
};
use tablecraft_access::editing::sql::RecoverableError;
use tablecraft_query::{QueryError, UpdateError};
use tablecraft_testkit::fixtures::access::{
    canonical_mixed_before_change as mixed_events, canonical_ordinary_content as ordinary,
    mutation_fixture,
};
use tablecraft_testkit::support::access::retained_target::assert_retained_target_observation;

fn review_update(
    worker: &crate::Worker,
    revision: u64,
    sql: &str,
) -> (crate::QueryUpdateReview, ReviewedQueryUpdate) {
    let (target, _) = inspect(worker, revision, sql);
    let Response::QueryUpdateReviewed { review, capability } = request(
        worker,
        Command::ReviewQueryUpdate {
            target,
            parameters: Vec::new(),
        },
    )
    .unwrap() else {
        panic!("expected a lease-free native SQL UPDATE review");
    };
    assert_eq!(review.document_revision, revision);
    assert_eq!(review.table, "Items");
    (review, capability)
}

fn save_and_acknowledge(
    capability: ReviewedQueryUpdate,
    bundle: std::path::PathBuf,
) -> crate::SaveReceipt {
    let saves = SaveWorker::new(|| {});
    let id = saves
        .start_query_update(capability, bundle.clone())
        .unwrap();
    let report = terminal(&saves);
    let SaveOutcome::Saved { receipt } = report.outcome else {
        panic!("expected completed native SQL save: {report:?}");
    };
    assert!(saves.acknowledge(id));
    assert!(!bundle.exists());
    receipt
}

#[test]
fn ace16_headers_save_and_reopen_against_independent_complete_image_overlays() {
    for header in [5, 6] {
        let mut source = ordinary::source(ordinary::Case::Extrema);
        source.bytes[20] = header;
        let table = &source.tables[0];
        let mut expected = source.bytes.clone();
        let base = table.data[0] as usize * 4096;
        let slot = u16::from_le_bytes(expected[base + 14..base + 16].try_into().unwrap());
        assert_eq!(slot & 0xe000, 0);
        let start = base + usize::from(slot);
        expected[start + 6..start + 10].copy_from_slice(&73_i32.to_le_bytes());

        let (directory, path, worker, revision) = opened(&source.bytes);
        let sql = format!(
            "UPDATE [{}] SET [{}]=73 WHERE [{}]=-2147483648",
            table.name, table.columns[1], table.columns[1]
        );
        let (target, _) = inspect(&worker, revision, &sql);
        let Response::QueryUpdateReviewed { review, capability } = request(
            &worker,
            Command::ReviewQueryUpdate {
                target,
                parameters: Vec::new(),
            },
        )
        .unwrap() else {
            panic!("expected ordinary ACE16 SQL UPDATE review");
        };
        assert_eq!(review.table, table.name);
        assert_eq!(review.document_revision, revision);
        assert_eq!(review.matched_rows, 1);
        assert_eq!(review.changed_rows, 1);
        assert_eq!(std::fs::read(&path).unwrap(), source.bytes);
        let receipt = save_and_acknowledge(capability, directory.path().join("ace-update.journal"));
        assert_eq!(receipt.matched_rows, Some(1));
        assert_eq!(receipt.changed_rows, Some(1));
        assert_eq!(std::fs::read(&path).unwrap(), expected);
        request(&worker, open(&path, "")).unwrap();
        let Response::Data { rows, .. } = request(
            &worker,
            Command::Page {
                table: table.name.clone(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("expected reopened ACE16 rows");
        };
        assert_eq!(rows[0][0].text, "1");
        assert_eq!(rows[0][1].text, "73");
        assert_eq!(rows[1][0].text, "2");
        assert_eq!(rows[1][1].text, "2147483647");
    }
}

#[test]
fn reviewed_multirow_update_holds_no_lease_and_saves_complete_independent_after_once() {
    let original = mutation_fixture::database();
    let expected = mutation_fixture::with_rows(&[(1, 10), (2, 25), (3, 35)]);
    let (directory, path, worker, revision) = opened(&original);
    let (review, capability) = review_update(
        &worker,
        revision,
        "UPDATE Items SET Score=Score+5 WHERE Id>=2",
    );
    assert_ne!(review.id, 0);
    assert_eq!(review.matched_rows, 2);
    assert_eq!(review.changed_rows, 2);
    assert_eq!(capability.source_path(), path);
    assert_eq!(std::fs::read(&path).unwrap(), original);

    // Waiting for confirmation retains immutable bytes, not the exclusive target.
    tablecraft_access::editing::offline::OfflineWriteSession::open(
        &path,
        crate::controls::SCOPE,
        &mut crate::controls::budget(),
        &|| false,
    )
    .unwrap()
    .release()
    .unwrap();
    let (_, second) = review_update(&worker, revision, "UPDATE Items SET Score=777 WHERE Id=1");
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("query-update.journal");
    let id = saves
        .start_query_update(capability, bundle.clone())
        .unwrap();
    let refused = saves
        .start_query_update(second, directory.path().join("second.journal"))
        .unwrap_err();
    assert_eq!(refused.kind, SaveStartFailure::Busy);
    assert!(refused.into_reviewed().is_some());

    // The read worker's subsequent SQL and cancellation cannot replay or replace
    // the original consuming native request on the separate save owner.
    let _ = request(
        &worker,
        Command::Query {
            options: Default::default(),
            sql: "SELECT 1/0".into(),
        },
    );
    worker.cancel();
    let report = terminal(&saves);
    assert_eq!(report.id, id);
    let SaveOutcome::Saved { receipt } = report.outcome else {
        panic!("expected completed native SQL save: {report:?}");
    };
    assert_eq!(receipt.matched_rows, Some(2));
    assert_eq!(receipt.changed_rows, Some(2));
    assert_eq!(saves.poll(), Some(report));
    assert!(!saves.cancel(id));
    assert!(saves.acknowledge(id));
    assert!(!bundle.exists());
    assert_eq!(std::fs::read(&path).unwrap(), expected);

    request(&worker, open(&path, "")).unwrap();
    let Response::Data { rows, .. } = request(
        &worker,
        Command::Page {
            table: "Items".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected reopened updated rows");
    };
    assert_eq!(
        super::row_texts(&rows),
        vec![vec!["1", "10"], vec!["2", "25"], vec!["3", "35"]]
    );
}

#[test]
fn protected_tail_change_refuses_before_journal_and_retains_exact_target_until_release() {
    let original = mutation_fixture::database();
    let (directory, path, worker, revision) = opened(&original);
    let (_, capability) = review_update(&worker, revision, "UPDATE Items SET Score=99 WHERE Id=2");
    let mut changed = original;
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();
    let bundle = directory.path().join("stale-query-update.journal");
    let saves = SaveWorker::new(|| {});
    let id = saves
        .start_query_update(capability, bundle.clone())
        .unwrap();
    let SaveOutcome::Failed(failure) = terminal(&saves).outcome else {
        panic!("expected exact original-source refusal");
    };
    assert_eq!(failure.kind, SaveFailureKind::SourceChanged);
    assert!(failure.can_release);
    assert!(!failure.can_recover);
    assert!(!saves.acknowledge(id));
    assert!(!bundle.exists());
    assert_retained_target_observation(&path, &changed);
    let release = saves.release(id).unwrap();
    assert!(matches!(
        terminal(&saves).outcome,
        SaveOutcome::Released {
            current_outcome: None,
            last_completed: None
        }
    ));
    assert!(saves.acknowledge(release));
    assert_eq!(std::fs::read(path).unwrap(), changed);
}

#[test]
fn inspected_identity_and_document_lifetime_reject_old_reviews_before_save() {
    let original = mutation_fixture::database();
    let (directory, path, worker, revision) = opened(&original);
    let (_other_directory, _other_path, other, other_revision) = opened(&original);
    let (target, _) = inspect(&worker, revision, "UPDATE Items SET Score=42 WHERE Id=2");
    assert_eq!(revision, other_revision);
    assert!(matches!(
        request(
            &other,
            Command::ReviewQueryUpdate {
                target: target.clone(),
                parameters: Vec::new(),
            }
        ),
        Err(Error::StaleQuery)
    ));
    let (_, capability) = review_update(&worker, revision, target.sql());
    request(&worker, open(&path, "")).unwrap();
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryUpdate {
                target,
                parameters: Vec::new()
            }
        ),
        Err(Error::StaleQuery)
    ));
    let saves = SaveWorker::new(|| {});
    let failure = saves
        .start_query_update(capability, directory.path().join("old.journal"))
        .unwrap_err();
    assert_eq!(failure.kind, SaveStartFailure::StaleDocument);
    assert!(failure.into_reviewed().is_some());
    assert!(!saves.is_busy());
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn zero_match_and_noop_reviews_keep_honest_counts_and_all_original_bytes() {
    for (sql, matched) in [
        ("UPDATE Items SET Score=99 WHERE Id=999", 0),
        ("UPDATE Items SET Score=Score WHERE Id>=2", 2),
    ] {
        let original = mutation_fixture::database();
        let (directory, path, worker, revision) = opened(&original);
        let (review, capability) = review_update(&worker, revision, sql);
        assert_eq!(review.matched_rows, matched);
        assert_eq!(review.changed_rows, 0);
        let receipt = save_and_acknowledge(capability, directory.path().join("empty.journal"));
        assert_eq!(receipt.matched_rows, Some(matched));
        assert_eq!(receipt.changed_rows, Some(0));
        assert_eq!(std::fs::read(path).unwrap(), original);
    }
}

#[test]
fn unsupported_actions_eager_errors_and_read_only_sources_never_write() {
    let original = mutation_fixture::database();
    let (directory, path, worker, revision) = opened(&original);
    for sql in [
        "SELECT * FROM Items",
        "UPDATE Items INNER JOIN Other ON Items.Id=Other.Id SET Score=1",
    ] {
        let (target, _) = inspect(&worker, revision, sql);
        assert!(matches!(
            request(
                &worker,
                Command::ReviewQueryUpdate {
                    target,
                    parameters: Vec::new()
                }
            ),
            Err(Error::QueryUpdate(RecoverableError::Shape(
                QueryError::Unsupported
            )))
        ));
    }
    let (target, _) = inspect(
        &worker,
        revision,
        "UPDATE Items SET Score=1 WHERE FALSE AND 1/0=0",
    );
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryUpdate {
                target,
                parameters: Vec::new()
            }
        ),
        Err(Error::QueryUpdate(RecoverableError::Plan(
            UpdateError::Query(QueryError::Evaluation(
                tablecraft_expressions::EvalError::DivisionByZero
            ))
        )))
    ));
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!crate::journal_path(&path).exists());

    // This integration derivative only tests the runtime read-only boundary;
    // independent engine crypto fixtures own encryption correctness separately.
    let sample_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/examples/Access_Example_VBA.accdb");
    let sample = std::fs::read(sample_path).unwrap();
    let encrypted = tablecraft_ace::rewrite_encryption(
        &mut std::io::Cursor::new(&sample),
        tablecraft_ace::EncryptionRewrite::Add {
            password: None,
            new_password: "test-only credential",
        },
        &mut crate::controls::budget(),
        &|| false,
    )
    .unwrap();
    let encrypted_path = directory.path().join("encoded.accdb");
    std::fs::write(&encrypted_path, encrypted.as_bytes()).unwrap();
    let Response::Opened {
        document_revision, ..
    } = request(&worker, open(&encrypted_path, "test-only credential")).unwrap()
    else {
        panic!("expected authenticated read-only native source");
    };
    let (target, _) = inspect(
        &worker,
        document_revision,
        "UPDATE Items SET Score=42 WHERE Id=2",
    );
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryUpdate {
                target,
                parameters: Vec::new()
            }
        ),
        Err(Error::Edit(crate::EditError::ReadOnlySource))
    ));
    assert_eq!(
        std::fs::read(&encrypted_path).unwrap(),
        encrypted.as_bytes()
    );
    assert!(!crate::journal_path(&encrypted_path).exists());
}

#[test]
fn stored_text_policy_selects_updated_rows_independently_of_database_default() {
    for header in [5, 6] {
        for field_version in [0, 1] {
            // The plain unindexed canonical15 ordinary fixture's full User-role
            // proof refuses legacy Text descriptors. Keep that native boundary;
            // this independently authored positive BeforeChange source already
            // qualifies variable siblings and preserves the real stored program.
            let source = mixed_events::source(header, mixed_events::Recipe::VariableManual);
            let mut original = source.before;
            let descriptor = 7 * 4096 + 63 + 25 + 11;
            assert_eq!(&original[descriptor..descriptor + 4], &[9, 4, 0, 1]);
            original[descriptor..descriptor + 4].copy_from_slice(&[9, 4, 0, field_version]);

            // This raw six-field source stores Id/Score before three variable
            // fields. Replace its first ten-byte UTF16 payload without moving
            // any footer, map, opaque sibling or program byte.
            let slot_at = 8 * 4096 + 14;
            let slot = u16::from_le_bytes(original[slot_at..slot_at + 2].try_into().unwrap());
            assert_eq!(slot & 0xe000, 0);
            let start = 8 * 4096 + usize::from(slot);
            assert_eq!(&original[start..start + 2], &6_u16.to_le_bytes());
            assert_eq!(&original[start + 2..start + 6], &1_i32.to_le_bytes());
            assert_eq!(&original[start + 6..start + 10], &1000_i32.to_le_bytes());
            assert_eq!(
                &original[start + 10..start + 20],
                &[0xa9, 3, 0xea, 0x96, 0x3d, 0xd8, 0x42, 0xde, 0x61, 0]
            );
            original[start + 10..start + 20]
                .copy_from_slice(&[0x80, 0, 0x80, 0, 0x80, 0, 0x80, 0, 0x80, 0]);

            let matched = usize::from(field_version == 0);
            let mut expected = original.clone();
            if matched == 1 {
                // The retained program adds Id and then Old.Score to the
                // supplied 77: 77 + 1 + 1000 = 1078, not merely the SQL value.
                expected[start + 6..start + 10].copy_from_slice(&1078_i32.to_le_bytes());
            }
            let (directory, path, worker, revision) = opened(&original);
            let (review, capability) = review_update(
                &worker,
                revision,
                "UPDATE Items SET Score=77 WHERE Id=1 AND TextValue<'\u{0301}'",
            );
            assert_eq!(review.matched_rows, matched);
            assert_eq!(review.changed_rows, matched);
            assert_eq!(std::fs::read(&path).unwrap(), original);
            let receipt =
                save_and_acknowledge(capability, directory.path().join("field-policy.journal"));
            assert_eq!(receipt.matched_rows, Some(matched));
            assert_eq!(receipt.changed_rows, Some(matched));
            assert_eq!(std::fs::read(&path).unwrap(), expected);
            request(&worker, open(&path, "")).unwrap();
            let Response::Data { rows, .. } = request(
                &worker,
                Command::Page {
                    table: "Items".into(),
                    offset: 0,
                },
            )
            .unwrap() else {
                panic!("expected field-policy source after save and reopen");
            };
            assert_eq!(rows[0][0].text, "1");
            assert_eq!(rows[0][5].text, if matched == 1 { "1078" } else { "1000" });
            assert_eq!(rows[1][5].text, "2000");
            assert_eq!(rows[2][5].text, "3000");
        }
    }
}

#[test]
fn declared_update_inputs_review_save_and_reopen_one_frozen_native_plan() {
    let original = mutation_fixture::database();
    let expected = mutation_fixture::with_rows(&[(1, 10), (2, 25), (3, 35)]);
    let (directory, path, worker, revision) = opened(&original);
    let sql = "PARAMETERS minimum LONG, increment LONG; UPDATE Items SET Score=Score+increment WHERE Id>=minimum";
    let (target, specs) = inspect(&worker, revision, sql);
    assert_eq!(specs.len(), 2);
    let mut parameters = vec![
        crate::ParameterInput {
            name: "minimum".into(),
            kind: "int32".into(),
            text: "2".into(),
            is_null: false,
        },
        crate::ParameterInput {
            name: "increment".into(),
            kind: "int32".into(),
            text: "5".into(),
            is_null: false,
        },
    ];
    let Response::QueryUpdateReviewed { review, capability } = request(
        &worker,
        Command::ReviewQueryUpdate {
            target,
            parameters: parameters.clone(),
        },
    )
    .unwrap() else {
        panic!("expected typed action review");
    };
    assert_eq!((review.matched_rows, review.changed_rows), (2, 2));
    assert_eq!(std::fs::read(&path).unwrap(), original);
    // The reviewed plan owns resolved native carriers, never this mutable input.
    parameters[1].text = "999".into();
    let receipt = save_and_acknowledge(capability, directory.path().join("parameters.journal"));
    assert_eq!(
        (receipt.matched_rows, receipt.changed_rows),
        (Some(2), Some(2))
    );
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    request(&worker, open(&path, "")).unwrap();
    let Response::Data { rows, .. } = request(
        &worker,
        Command::Page {
            table: "Items".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected reopened typed action rows");
    };
    assert_eq!(rows[1][1].text, "25");
    assert_eq!(rows[2][1].text, "35");
}

#[test]
fn invalid_action_inputs_refuse_before_unreadable_native_data() {
    let mut original = mutation_fixture::database();
    original[12 * 4096] = 0;
    let (_directory, path, worker, revision) = opened(&original);
    let (target, _) = inspect(
        &worker,
        revision,
        "PARAMETERS replacement LONG; UPDATE Items SET Score=replacement",
    );
    let input = crate::ParameterInput {
        name: "replacement".into(),
        kind: "int32".into(),
        text: "42".into(),
        is_null: false,
    };
    for parameters in [
        Vec::new(),
        vec![crate::ParameterInput {
            kind: "text".into(),
            ..input.clone()
        }],
        vec![input.clone(), input.clone()],
        vec![crate::ParameterInput {
            name: "other".into(),
            ..input.clone()
        }],
    ] {
        assert!(matches!(
            request(
                &worker,
                Command::ReviewQueryUpdate {
                    target: target.clone(),
                    parameters,
                }
            ),
            Err(Error::ParameterInput)
        ));
    }
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryUpdate {
                target,
                parameters: vec![crate::ParameterInput {
                    text: "DELETE FROM Items".into(),
                    ..input
                }],
            }
        ),
        Err(Error::InvalidScalarInput)
    ));
    let (limited, _) = inspect(
        &worker,
        revision,
        "PARAMETERS replacement TEXT(2); UPDATE Items SET Score=1 WHERE FALSE",
    );
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryUpdate {
                target: limited,
                parameters: vec![crate::ParameterInput {
                    name: "replacement".into(),
                    kind: "text".into(),
                    text: "abc".into(),
                    is_null: false
                }],
            }
        ),
        Err(Error::ParameterInput)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!crate::journal_path(&path).exists());
}

mod joined;

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

mod read_source;

#[cfg(feature = "ace16-create")]
mod retained_batch;

#[cfg(feature = "ace16-create")]
mod indexed_long;

#[cfg(feature = "ace16-create")]
mod formats;
mod like;
#[cfg(feature = "ace16-create")]
mod math;

#[cfg(feature = "ace16-create")]
mod date_intervals;

#[cfg(feature = "ace16-create")]
mod currency_batch;
