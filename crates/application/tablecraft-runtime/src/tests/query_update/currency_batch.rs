//! Currency batches preserve exact scaled values and every protected companion.
use super::{assert_image, inspect, open, opened, request, save_and_acknowledge};
use crate::{Command, Error, Response};
use tablecraft_access::editing::sql::RecoverableError;
use tablecraft_expressions::EvalError;
use tablecraft_query::{QueryError, UpdateError};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::currency::batch;

#[test]
fn currency_null_noop_empty_and_singleton_updates_save_exact_original_bound_images() {
    for (sql, values, matched, changed) in [
        (
            "UPDATE CurrencyValues SET KeyValue=CCur(Nz(KeyValue,0)+1.25)",
            [Some(24845), Some(12500)],
            2,
            2,
        ),
        (
            "UPDATE CurrencyValues SET KeyValue=NULL",
            [None, None],
            2,
            1,
        ),
        (
            "UPDATE CurrencyValues SET KeyValue=KeyValue",
            [Some(12345), None],
            2,
            0,
        ),
        (
            "UPDATE CurrencyValues SET KeyValue=CCur(5.25) WHERE KeyValue>CCur(100)",
            [Some(12345), None],
            0,
            0,
        ),
        (
            "UPDATE CurrencyValues SET KeyValue=CCur(KeyValue+1.25) WHERE KeyValue IS NOT NULL",
            [Some(24845), None],
            1,
            1,
        ),
    ] {
        let source = batch::values(&[Some(12345), None], &values);
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
        .unwrap_or_else(|error| panic!("Currency review {sql}: {error:?}")) else {
            panic!("one consuming Currency review");
        };
        assert_eq!(review.table, source.table);
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
        assert_image(&std::fs::read(&path).unwrap(), &source.expected);
        request(&worker, open(&path, "")).unwrap();
        assert!(matches!(
            request(
                &worker,
                Command::ReviewQueryUpdate {
                    target: stale,
                    parameters: Vec::new(),
                },
            ),
            Err(Error::StaleQuery)
        ));

        let Response::Data { rows, .. } = request(
            &worker,
            Command::Query {
                options: Default::default(),
                sql: "SELECT IIf(IsNull(KeyValue),NULL,CLng(Nz(KeyValue,0)*10000)) AS ScaledUnits, Amount, Label FROM CurrencyValues ORDER BY Guard".into(),
            },
        )
        .unwrap() else {
            panic!("fresh exact Currency query");
        };
        assert_eq!(rows.len(), values.len());
        for (row, value) in rows.iter().zip(values) {
            assert_eq!(
                row[0].text,
                value.map_or_else(|| "NULL".to_string(), |value| value.to_string())
            );
        }
        assert_image(&std::fs::read(path).unwrap(), &source.expected);
    }
}

#[test]
fn both_historically_missing_currency_fields_materialize_in_one_reviewed_save() {
    let source = batch::materializing();
    let (_directory, path, worker, revision) = opened(&source.before);
    let (target, _) = inspect(
        &worker,
        revision,
        "UPDATE Table1 SET ExtraCurrency=CCur(Nz(Score,0)/10+0.5)",
    );
    let Response::QueryUpdateReviewed { review, capability } = request(
        &worker,
        Command::ReviewQueryUpdate {
            target,
            parameters: Vec::new(),
        },
    )
    .unwrap() else {
        panic!("historical Currency batch materialization");
    };
    assert_eq!((review.matched_rows, review.changed_rows), (2, 2));
    assert_image(&std::fs::read(&path).unwrap(), &source.before);
    save_and_acknowledge(capability, crate::journal_path(&path));
    assert_image(&std::fs::read(&path).unwrap(), &source.expected);
    request(&worker, open(&path, "")).unwrap();
    let Response::Data { rows, .. } = request(
        &worker,
        Command::Query {
            options: Default::default(),
            sql: "SELECT CLng(ExtraCurrency*10000) AS ScaledUnits FROM Table1 ORDER BY ID".into(),
        },
    )
    .unwrap() else {
        panic!("fresh materialized exact Currency values");
    };
    assert_eq!(rows[0][0].text, "22000");
    assert_eq!(rows[1][0].text, "5000");
    assert_image(&std::fs::read(path).unwrap(), &source.expected);
}

#[test]
fn later_currency_overflow_never_publishes_the_first_valid_replacement() {
    let source = batch::values(&[Some(0), Some(i64::MAX)], &[Some(0), Some(i64::MAX)]);
    let (_directory, path, worker, revision) = opened(&source.before);
    let (target, _) = inspect(
        &worker,
        revision,
        "UPDATE CurrencyValues SET KeyValue=KeyValue+CCur(1)",
    );
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryUpdate {
                target,
                parameters: Vec::new(),
            },
        ),
        Err(Error::QueryUpdate(RecoverableError::Plan(
            UpdateError::Query(QueryError::Evaluation(EvalError::Overflow))
        )))
    ));
    assert_image(&std::fs::read(&path).unwrap(), &source.before);
    assert!(!crate::journal_path(&path).exists());
    let Response::Data { rows, .. } = request(
        &worker,
        Command::Query {
            options: Default::default(),
            sql: "SELECT KeyValue=CCur(0) AS FirstOriginal, KeyValue>CCur(1) AS SecondOriginal FROM CurrencyValues ORDER BY Guard".into(),
        },
    )
    .unwrap() else {
        panic!("worker remains healthy after late Currency overflow");
    };
    assert_eq!(rows.len(), 2);
    assert_image(&std::fs::read(path).unwrap(), &source.before);
}
