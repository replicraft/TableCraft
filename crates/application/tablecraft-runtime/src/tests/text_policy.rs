//! Actual host policy wiring, separately from native weight qualification.
use super::{open, query, request, row_texts};
use crate::{Command, Error, Response, SavedQueryTarget, Worker};
use tablecraft_expressions::EvalError;
use tablecraft_query::QueryError;
use tablecraft_testkit::fixtures::access::saved_queries::{
    Fragment, QUERY_ID, database_with_query_flags,
};

pub(super) fn database_order(bytes: &mut [u8], order: [u8; 4]) {
    // Independently authored header bytes; never obtain these from its decoder.
    for (position, mask) in [1, 27, 152, 253].into_iter().enumerate() {
        bytes[110 + position] = order[position] ^ mask;
    }
}

fn rows(worker: &Worker, sql: &str) -> Vec<Vec<crate::Cell>> {
    let Response::Data {
        rows,
        source_revision,
        ..
    } = request(worker, query(sql)).unwrap()
    else {
        panic!("expected buffered query results");
    };
    assert_eq!(source_revision, None);
    rows
}

#[test]
fn native_text_policies_reach_predicates_groups_and_explicit_string_modes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("text.accdb");
    let mut original =
        include_bytes!("../../../../../docs/examples/Access_Example_VBA.accdb").to_vec();
    database_order(&mut original, [9, 4, 0, 0]);
    std::fs::write(&path, &original).unwrap();
    let worker = Worker::new(|| {});
    request(&worker, open(&path, "")).unwrap();

    let actual = rows(
        &worker,
        "SELECT 'a'='A ' AS Same, StrComp('A','a',0) AS BinaryValue, StrComp('a','A',2) AS DatabaseValue",
    );
    assert_eq!(row_texts(&actual), [["true", "-1", "0"]]);

    let actual = rows(
        &worker,
        "SELECT Label, Count(*) AS N FROM (SELECT 'a' AS Label UNION ALL SELECT 'A ') AS Labels GROUP BY Label",
    );
    assert_eq!(row_texts(&actual), [["a", "2"]]);

    // A stored field retains its own descriptor, rather than inheriting a
    // host-wide comparison override from the database header.
    let actual = rows(
        &worker,
        "SELECT TOP 1 NBR_VALUE='2' AS Same FROM ROULETTE_TB ORDER BY ORDR_NBR",
    );
    assert_eq!(row_texts(&actual), [["true"]]);

    // This actual database declares ANSI-89; it does not supply a VBA module's
    // omitted Option Compare setting.
    assert_eq!(
        row_texts(&rows(&worker, "SELECT 'abc' LIKE 'a*'")),
        [["true"]]
    );
    assert!(request(&worker, query("SELECT StrComp('a','A')")).is_err());
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn native_wildcard_mode_uses_only_qualified_database_settings() {
    use tablecraft_testkit::fixtures::access::database_properties;

    for mode in [Some(0_i32), Some(1), Some(2), None] {
        let raw_mode = mode.unwrap_or(0).to_le_bytes();
        let properties = match mode {
            Some(_) => database_properties::query_modes(&[(1, 4, &raw_mode)]),
            None => database_properties::query_modes(&[]),
        };
        let mut original = database_properties::database(Some(&properties));
        database_order(&mut original, [9, 4, 0, 0]);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("wildcards.mdb");
        std::fs::write(&path, &original).unwrap();
        let worker = Worker::new(|| {});
        request(&worker, open(&path, "")).unwrap();

        assert_eq!(row_texts(&rows(&worker, "SELECT 40+2 AS N")), [["42"]]);
        let sql = "SELECT 'Apple' LIKE 'a*' AS [Star], 'Apple' LIKE 'a%' AS [Percent]";
        match mode {
            Some(0) => assert_eq!(row_texts(&rows(&worker, sql)), [["true", "false"]]),
            Some(1) => assert_eq!(row_texts(&rows(&worker, sql)), [["false", "true"]]),
            _ => assert!(matches!(
                request(&worker, query(sql)),
                Err(Error::Query(QueryError::Evaluation(
                    EvalError::LikeModeRequired
                )))
            )),
        }
        assert_eq!(std::fs::read(path).unwrap(), original);
    }
}

#[test]
fn unknown_default_does_not_block_numeric_queries_or_known_field_comparison() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("unknown.accdb");
    let mut original =
        include_bytes!("../../../../../docs/examples/Access_Example_VBA.accdb").to_vec();
    database_order(&mut original, [31, 4, 0, 1]);
    std::fs::write(&path, &original).unwrap();
    let worker = Worker::new(|| {});
    request(&worker, open(&path, "")).unwrap();

    assert_eq!(row_texts(&rows(&worker, "SELECT 40+2 AS N")), [["42"]]);
    let actual = rows(
        &worker,
        "SELECT TOP 1 NBR_VALUE, NBR_VALUE='2' AS Same FROM ROULETTE_TB ORDER BY ORDR_NBR",
    );
    assert_eq!(row_texts(&actual), [["2", "true"]]);
    for sql in ["SELECT 'a'='A'", "SELECT StrComp('a','A',2)"] {
        assert!(matches!(
            request(&worker, query(sql)),
            Err(Error::Query(QueryError::Evaluation(
                EvalError::CollationRequired
            )))
        ));
    }
    assert_eq!(
        row_texts(&rows(&worker, "SELECT StrComp('A','a',0) AS N")),
        [["-1"]]
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn saved_read_uses_the_same_database_default_without_activating_inspection() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("saved.accdb");
    let mut original = database_with_query_flags(
        &[Fragment {
            object: Some(QUERY_ID),
            attribute: Some(6),
            expression: Some("'a'='A'"),
            order: Some(&[0, 0, 0, 1]),
            ..Default::default()
        }],
        0,
    );
    database_order(&mut original, [9, 4, 0, 0]);
    std::fs::write(&path, &original).unwrap();
    let worker = Worker::new(|| {});
    let Response::Opened {
        document_revision,
        queries,
        ..
    } = request(&worker, open(&path, "")).unwrap()
    else {
        panic!("expected catalog inventory");
    };
    let selected = queries.iter().find(|query| query.name == "Saved").unwrap();
    let target = SavedQueryTarget {
        document_revision,
        object_id: selected.object_id,
        name: selected.name.clone(),
    };
    assert!(matches!(
        request(
            &worker,
            Command::InspectSavedQuery {
                target: target.clone(),
            },
        )
        .unwrap(),
        Response::SavedQuery { .. }
    ));
    let Response::Data { rows, .. } = request(
        &worker,
        Command::RunSavedQuery {
            options: Default::default(),
            target: target.clone(),
            parameters: Vec::new(),
        },
    )
    .unwrap() else {
        panic!("expected saved query results");
    };
    assert_eq!(row_texts(&rows), [["true"]]);
    assert_eq!(std::fs::read(&path).unwrap(), original);

    database_order(&mut original, [31, 4, 0, 1]);
    std::fs::write(&path, &original).unwrap();
    let Response::Opened {
        document_revision, ..
    } = request(&worker, open(&path, "")).unwrap()
    else {
        panic!("expected reopened inventory");
    };
    let target = SavedQueryTarget {
        document_revision,
        ..target
    };
    assert!(matches!(
        request(
            &worker,
            Command::InspectSavedQuery {
                target: target.clone(),
            },
        )
        .unwrap(),
        Response::SavedQuery { .. }
    ));
    assert!(matches!(
        request(
            &worker,
            Command::RunSavedQuery {
                options: Default::default(),
                target,
                parameters: Vec::new(),
            },
        ),
        Err(Error::Query(QueryError::Evaluation(
            EvalError::CollationRequired
        )))
    ));
    assert_eq!(std::fs::read(path).unwrap(), original);
}
