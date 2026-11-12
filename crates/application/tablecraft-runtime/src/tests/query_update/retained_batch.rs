//! Retained same-column UPDATE stages every original row before one native finish.
use super::{assert_image, inspect, open, opened, request, save_and_acknowledge};
use crate::{Command, Error, Response};
use tablecraft_access::editing::sql::RecoverableError;
use tablecraft_query::{QueryError, UpdateError};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    currency::mixed, field_changes, fields, joined_update, long_column_batch,
};

#[test]
fn retained_two_row_updates_null_noop_and_zero_save_complete_original_bound_images() {
    for (sql, values, matched, changed) in [
        (
            "UPDATE Table1 SET Score=CLng(Nz(Score,0)+5)",
            [Some(22), Some(5)],
            2,
            2,
        ),
        ("UPDATE Table1 SET Score=NULL", [None, None], 2, 1),
        ("UPDATE Table1 SET Score=Score", [Some(17), None], 2, 0),
        (
            "UPDATE Table1 SET Score=25 WHERE ID<0",
            [Some(17), None],
            0,
            0,
        ),
    ] {
        let source = long_column_batch::two_rows();
        let expected = if values == [Some(22), Some(5)] {
            source.expected
        } else {
            let expected =
                field_changes::update_score(&source.before, source.definition, 0, values[0]);
            field_changes::update_score(&expected, source.definition, 1, values[1])
        };
        let (_directory, path, worker, revision) = opened(&source.before);
        let (target, _) = inspect(&worker, revision, sql);
        let (stale, _) = inspect(&worker, revision, sql);
        let Response::QueryUpdateReviewed { review, capability } = request(
            &worker,
            Command::ReviewQueryUpdate {
                target,
                parameters: Vec::new(),
            },
        )
        .unwrap_or_else(|error| panic!("retained batch review: {sql}: {error:?}")) else {
            panic!("expected retained batch review");
        };
        assert_eq!(
            (review.matched_rows, review.changed_rows),
            (matched, changed)
        );
        assert_image(&std::fs::read(&path).unwrap(), &source.before);
        assert!(!crate::journal_path(&path).exists());
        let receipt = save_and_acknowledge(capability, crate::journal_path(&path));
        assert_eq!(
            (receipt.matched_rows, receipt.changed_rows),
            (Some(matched), Some(changed))
        );
        assert!(receipt.schema_change.is_none());
        assert_image(&std::fs::read(&path).unwrap(), &expected);
        request(&worker, open(&path, "")).unwrap();
        assert!(matches!(
            request(
                &worker,
                Command::ReviewQueryUpdate {
                    target: stale,
                    parameters: Vec::new(),
                }
            ),
            Err(Error::StaleQuery)
        ));
        let Response::Data { columns, rows, .. } = request(
            &worker,
            Command::Page {
                table: "Table1".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("fresh retained rows");
        };
        assert_eq!(columns[1].native_number, Some(1));
        for (row, value) in rows.iter().zip(values) {
            assert_eq!(
                row[1].text,
                value.map_or_else(|| "NULL".to_string(), |v| v.to_string())
            );
        }
        assert_image(&std::fs::read(path).unwrap(), &expected);
    }
}

#[test]
fn retained_batch_preserves_same_record_currency_unicode_and_nullable_companions() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let mut before = mixed::append_table(&seed, "Measurements");
    // Establish both logical roots before variable text growth appends hidden
    // terminal slots; physical slot 1 must remain the second live root.
    for _ in 0..2 {
        before = mixed::append(&before, definition);
    }
    for (root, score, amount, label) in [
        (0, Some(17), Some(12345), Some("日本🦀")),
        (1, None, None, Some("")),
    ] {
        before = mixed::update_number(&before, definition, root, score);
        before = mixed::update_currency(&before, definition, root, amount);
        before = mixed::update_text(&before, definition, root, label);
    }
    let expected = mixed::update_number(&before, definition, 0, Some(22));
    let expected = mixed::update_number(&expected, definition, 1, Some(5));
    let (_directory, path, worker, revision) = opened(&before);
    let (target, _) = inspect(
        &worker,
        revision,
        "UPDATE Measurements SET Score=CLng(Nz(Score,0)+5)",
    );
    let Response::QueryUpdateReviewed { review, capability } = request(
        &worker,
        Command::ReviewQueryUpdate {
            target,
            parameters: Vec::new(),
        },
    )
    .unwrap() else {
        panic!("expected mixed retained batch");
    };
    assert_eq!((review.matched_rows, review.changed_rows), (2, 2));
    assert_image(&std::fs::read(&path).unwrap(), &before);
    save_and_acknowledge(capability, crate::journal_path(&path));
    assert_image(&std::fs::read(&path).unwrap(), &expected);
    request(&worker, open(&path, "")).unwrap();
    assert_image(&std::fs::read(path).unwrap(), &expected);
}

#[test]
fn later_assignment_failure_discards_the_complete_retained_batch_before_native_finish() {
    let source = joined_update::source();
    let (_directory, path, worker, revision) = opened(&source.bytes);
    let (target, _) = inspect(&worker, revision, "UPDATE Table1 SET Score=10 \\ (2-ID)");
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryUpdate {
                target,
                parameters: Vec::new(),
            }
        ),
        Err(Error::QueryUpdate(RecoverableError::Plan(
            UpdateError::Query(QueryError::Evaluation(
                tablecraft_expressions::EvalError::DivisionByZero
            ))
        )))
    ));
    assert_image(&std::fs::read(&path).unwrap(), &source.bytes);
    assert!(!crate::journal_path(&path).exists());
}
