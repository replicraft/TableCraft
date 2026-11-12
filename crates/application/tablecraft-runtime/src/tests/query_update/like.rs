//! Native criteria consume the captured database mode before any reviewed save.
use super::{assert_image, inspect, open, opened, request, save_and_acknowledge};
use crate::{Command, Error, Response, SaveOutcome, SaveWorker};
use tablecraft_access::editing::sql::RecoverableError;
use tablecraft_expressions::EvalError;
use tablecraft_query::{QueryError, UpdateError};
use tablecraft_testkit::fixtures::access::{
    canonical_retained_add_table::{
        currency::mixed, deletes, field_changes, fields, joined_update,
    },
    database_properties,
};

fn source(mode: Option<i32>) -> (Vec<u8>, u32) {
    let source = joined_update::source();
    let bytes = match mode {
        Some(mode) => database_properties::with_native17_properties(
            &source.bytes,
            &database_properties::name_auto_correct_disabled_with_query_mode(mode),
        ),
        None => source.bytes,
    };
    (bytes, source.target_definition)
}

#[cfg(feature = "ace16-create")]
fn native_image(before: &[u8]) -> tablecraft_ace::NativeImage {
    tablecraft_ace::NativeImage::capture_retained(
        &mut std::io::Cursor::new(before),
        crate::controls::SCOPE,
        &mut crate::controls::budget(),
        &|| false,
    )
    .unwrap()
}

#[cfg(feature = "ace16-create")]
fn require_native_update_source(before: &[u8], table: &str) {
    let observed = native_image(before).check_native_retained_cell_updates_table(
        table,
        &mut crate::controls::budget(),
        &|| false,
    );
    assert!(
        observed.is_ok(),
        "LIKE fixture native source for {table}: {observed:?}"
    );
}

#[test]
fn update_nullable_text_like_binds_native_ansi_modes_and_original_matching_handles() {
    for (mode, pattern, matched) in [
        (None, "1*", 1),
        (None, "1%", 0),
        (Some(0), "1*", 1),
        (Some(0), "1%", 0),
        (Some(1), "1%", 1),
        (Some(1), "1*", 0),
    ] {
        let (before, definition) = source(mode);
        #[cfg(feature = "ace16-create")]
        require_native_update_source(&before, "Table1");
        let expected = if matched == 0 {
            before.clone()
        } else {
            field_changes::update_score(&before, definition, 0, Some(25))
        };
        let (_directory, path, worker, revision) = opened(&before);
        let sql = format!(
            "UPDATE Table1 SET Score=25 WHERE IIf(Score IS NULL,NULL,'17') LIKE '{pattern}'"
        );
        let (target, _) = inspect(&worker, revision, &sql);
        let Response::QueryUpdateReviewed { review, capability } = request(
            &worker,
            Command::ReviewQueryUpdate {
                target,
                parameters: Vec::new(),
            },
        )
        .unwrap_or_else(|error| panic!("mode {mode:?}: {sql}: {error:?}")) else {
            panic!("native UPDATE review");
        };
        assert_eq!(
            (review.matched_rows, review.changed_rows),
            (matched, matched)
        );
        assert_image(&std::fs::read(&path).unwrap(), &before);
        assert!(!crate::journal_path(&path).exists());
        let receipt = save_and_acknowledge(capability, crate::journal_path(&path));
        assert_eq!(
            (receipt.matched_rows, receipt.changed_rows),
            (Some(matched), Some(matched))
        );
        assert_image(&std::fs::read(&path).unwrap(), &expected);
        request(&worker, open(&path, "")).unwrap();
        let Response::Data { rows, .. } = request(
            &worker,
            Command::Page {
                table: "Table1".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("fresh target rows");
        };
        assert_eq!(rows[0][1].text, if matched == 0 { "17" } else { "25" });
        assert_eq!(rows[1][1].text, "NULL");
        assert_image(&std::fs::read(&path).unwrap(), &expected);
    }
}

#[test]
fn delete_not_like_preserves_null_unknown_and_exact_native_after_in_both_modes() {
    for (mode, pattern) in [(0, "z*"), (1, "z%")] {
        let (before, definition) = source(Some(mode));
        #[cfg(feature = "ace16-create")]
        require_native_update_source(&before, "Table1");
        let expected = deletes::after(&before, definition, &[0]);
        let (_directory, path, worker, revision) = opened(&before);
        let sql =
            format!("DELETE FROM Table1 WHERE IIf(Score IS NULL,NULL,'17') NOT LIKE '{pattern}'");
        let (target, _) = inspect(&worker, revision, &sql);
        let Response::QueryDeleteReviewed { review, capability } = request(
            &worker,
            Command::ReviewQueryDelete {
                target,
                parameters: Vec::new(),
            },
        )
        .unwrap() else {
            panic!("native DELETE review");
        };
        assert_eq!((review.matched_rows, review.deleted_rows), (1, 1));
        assert_image(&std::fs::read(&path).unwrap(), &before);
        let saves = SaveWorker::new(|| {});
        let id = saves
            .start_query_delete(capability, crate::journal_path(&path))
            .unwrap();
        let SaveOutcome::Saved { receipt } = super::terminal(&saves).outcome else {
            panic!("completed DELETE save");
        };
        assert_eq!(
            (receipt.matched_rows, receipt.changed_rows),
            (Some(1), Some(1))
        );
        assert!(saves.acknowledge(id));
        assert_image(&std::fs::read(&path).unwrap(), &expected);
        request(&worker, open(&path, "")).unwrap();
        let Response::Data { rows, .. } = request(
            &worker,
            Command::Page {
                table: "Table1".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("fresh survivor");
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][1].text, "NULL");
        assert_image(&std::fs::read(&path).unwrap(), &expected);
    }
}

#[test]
fn unknown_native_mode_and_eager_late_errors_never_issue_a_save_capability() {
    for mode in [Some(2), Some(0), Some(1)] {
        let (before, _) = source(mode);
        let (_directory, path, worker, revision) = opened(&before);
        let (sql, expected) = match mode {
            Some(2) => (
                "UPDATE Table1 SET Score=25 WHERE '17' LIKE '1*'".to_string(),
                QueryError::Unsupported,
            ),
            _ => {
                let pattern = if mode == Some(0) { "1*" } else { "1%" };
                (
                    format!(
                        "UPDATE Table1 SET Score=25 WHERE '17' LIKE IIf(ID=2,10 \\ (2-ID),'{pattern}')"
                    ),
                    QueryError::Evaluation(EvalError::DivisionByZero),
                )
            }
        };
        let (target, _) = inspect(&worker, revision, &sql);
        let result = request(
            &worker,
            Command::ReviewQueryUpdate {
                target,
                parameters: Vec::new(),
            },
        );
        let actual = match result {
            Err(Error::QueryUpdate(RecoverableError::Plan(UpdateError::Query(actual)))) => actual,
            Err(error) => panic!("unexpected action error for mode {mode:?}, {sql}: {error:?}"),
            Ok(_) => panic!("failed criteria issued a capability for mode {mode:?}: {sql}"),
        };
        assert_eq!(actual, expected, "mode {mode:?}: {sql}");
        #[cfg(feature = "ace16-create")]
        if mode == Some(2) {
            let image = native_image(&before);
            assert_eq!(
                tablecraft_access::editing::sql::native_mutation_query_mode(
                    &image,
                    tablecraft_sql::parse_query(&sql, tablecraft_sql::ParseLimits::default())
                        .unwrap()
                        .query(),
                    &mut crate::controls::budget(),
                    &mut crate::controls::budget(),
                    &|| false,
                )
                .unwrap(),
                Some(tablecraft_ace::DatabaseQueryMode::Unknown),
                "unknown setting remains distinct from absent metadata"
            );
            assert!(
                image
                    .check_native_retained_cell_updates_table(
                        "Table1",
                        &mut crate::controls::budget(),
                        &|| false
                    )
                    .is_err(),
                "unknown setting is not native source authority"
            );
        }
        assert_image(&std::fs::read(&path).unwrap(), &before);
        assert!(!crate::journal_path(&path).exists());
    }
}

#[test]
fn stored_text_criteria_keep_native_policy_and_typed_companions_through_review_save() {
    for (mode, pattern) in [(0_i32, "a*"), (1_i32, "a%")] {
        let seed = fields::source(fields::Case::FirstNumber).bytes;
        let definition = (seed.len() / 4096) as u32;
        let mut before = mixed::append_table(&seed, "Measurements");
        for _ in 0..2 {
            before = mixed::append(&before, definition);
        }
        before = mixed::update_number(&before, definition, 0, Some(17));
        before = mixed::update_currency(&before, definition, 0, Some(12345));
        before = mixed::update_text(&before, definition, 0, Some("Apple"));
        before = mixed::update_number(&before, definition, 1, None);
        before = mixed::update_currency(&before, definition, 1, None);
        let properties = database_properties::name_auto_correct_disabled_with_query_mode(mode);
        let before = database_properties::with_native17_properties(&before, &properties);
        #[cfg(feature = "ace16-create")]
        require_native_update_source(&before, "Measurements");
        let expected = mixed::update_number(&before, definition, 0, Some(25));
        let (_directory, path, worker, revision) = opened(&before);
        let sql = format!("UPDATE Measurements SET Score=25 WHERE Label LIKE '{pattern}'");
        let (target, _) = inspect(&worker, revision, &sql);
        let Response::QueryUpdateReviewed { review, capability } = request(
            &worker,
            Command::ReviewQueryUpdate {
                target,
                parameters: Vec::new(),
            },
        )
        .unwrap() else {
            panic!("stored Text criteria review");
        };
        assert_eq!((review.matched_rows, review.changed_rows), (1, 1));
        assert_image(&std::fs::read(&path).unwrap(), &before);
        save_and_acknowledge(capability, crate::journal_path(&path));
        assert_image(&std::fs::read(&path).unwrap(), &expected);
        request(&worker, open(&path, "")).unwrap();
        let Response::Data { rows, .. } = request(
            &worker,
            Command::Page {
                table: "Measurements".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("fresh typed companions");
        };
        assert_eq!(rows[0][1].text, "25");
        assert_eq!(rows[0][2].text, "1.2345");
        assert_eq!(rows[0][3].text, "Apple");
        assert_eq!(rows[1][1].text, "NULL");
        assert_eq!(rows[1][3].text, "NULL");
        assert_image(&std::fs::read(&path).unwrap(), &expected);
    }
}

#[test]
fn conflicting_stored_text_policies_refuse_joined_like_without_source_effects() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let before = mixed::append_table(&seed, "Measurements");
    let before = mixed::append(&before, definition);
    let before = mixed::update_text(&before, definition, 0, Some("Apple"));
    let other = (before.len() / 4096) as u32;
    let before = mixed::append_table(&before, "Patterns");
    let before = mixed::append(&before, other);
    let mut before = mixed::update_text(&before, other, 0, Some("a*"));
    // Literal four-field declaration: Label native 3 begins at TDEF+150;
    // Text sort bytes begin eleven bytes into its physical descriptor.
    let sort = other as usize * 4096 + 150 + 11;
    assert_eq!(&before[sort..sort + 4], &[9, 4, 0, 1]);
    before[sort..sort + 4].copy_from_slice(&[9, 4, 0, 0]);
    let properties = database_properties::name_auto_correct_disabled_with_query_mode(0);
    let before = database_properties::with_native17_properties(&before, &properties);
    let (_directory, path, worker, revision) = opened(&before);
    let sql = "UPDATE Measurements AS a INNER JOIN Patterns AS b ON a.ID=b.ID SET a.Score=25 WHERE a.Label LIKE b.Label";
    let (target, _) = inspect(&worker, revision, sql);
    let result = request(
        &worker,
        Command::ReviewQueryUpdate {
            target,
            parameters: Vec::new(),
        },
    );
    let actual = match result {
        Err(Error::QueryUpdate(RecoverableError::Plan(UpdateError::Query(actual)))) => actual,
        Err(error) => panic!("unexpected joined LIKE action error: {error:?}"),
        Ok(_) => panic!("conflicting stored policies issued a capability"),
    };
    assert_eq!(actual, QueryError::CollationConflict);
    assert_image(&std::fs::read(&path).unwrap(), &before);
    assert!(!crate::journal_path(&path).exists());
}

#[test]
fn numeric_like_requires_an_explicit_number_format_before_update_or_delete_review() {
    for mode in [0, 1] {
        let (before, _) = source(Some(mode));
        #[cfg(feature = "ace16-create")]
        require_native_update_source(&before, "Table1");
        let (_directory, path, worker, revision) = opened(&before);
        for delete in [false, true] {
            let sql = if delete {
                "DELETE FROM Table1 WHERE Score LIKE '1*'"
            } else {
                "UPDATE Table1 SET Score=25 WHERE Score LIKE '1*'"
            };
            let (target, _) = inspect(&worker, revision, sql);
            let command = if delete {
                Command::ReviewQueryDelete {
                    target,
                    parameters: Vec::new(),
                }
            } else {
                Command::ReviewQueryUpdate {
                    target,
                    parameters: Vec::new(),
                }
            };
            let actual = match request(&worker, command) {
                Err(Error::QueryUpdate(RecoverableError::Plan(UpdateError::Query(actual)))) => {
                    actual
                }
                Err(Error::QueryDelete(
                    tablecraft_access::editing::sql::RecoverableDeleteError::Plan(
                        tablecraft_query::DeleteError::Query(actual),
                    ),
                )) => actual,
                Err(error) => panic!("unexpected numeric LIKE failure for {sql}: {error:?}"),
                Ok(_) => panic!("numeric LIKE invented a number-format policy for {sql}"),
            };
            assert_eq!(
                actual,
                QueryError::Evaluation(EvalError::NumberFormatRequired),
                "mode {mode}: {sql}"
            );
            assert_image(&std::fs::read(&path).unwrap(), &before);
            assert!(!crate::journal_path(&path).exists());
        }
    }
}
