//! INSERT consumes the actual saved CREATE and indexed UPDATE images.
mod select;

use super::{
    assert_image, create, create_definition, inspect, open, opened, request, save_and_acknowledge,
    terminal,
};
use crate::{Command, Error, InsertInput, InsertTarget, Response, SaveOutcome, SaveWorker, Worker};
use std::path::Path;
use tablecraft_testkit::fixtures::access::secondary_index::normal::inserting;

fn update(worker: &Worker, path: &Path, revision: u64, sql: &str, expected: &[u8]) -> u64 {
    let (target, _) = inspect(worker, revision, sql);
    let Response::QueryUpdateReviewed { capability, .. } = request(
        worker,
        Command::ReviewQueryUpdate {
            target,
            parameters: Vec::new(),
        },
    )
    .unwrap() else {
        panic!("actual indexed update review");
    };
    let receipt = save_and_acknowledge(capability, crate::journal_path(path));
    assert!(receipt.schema_change.is_none());
    assert_image(&std::fs::read(path).unwrap(), expected);
    let Response::Opened {
        document_revision, ..
    } = request(worker, open(path, "")).unwrap()
    else {
        panic!("fresh indexed update reopen");
    };
    document_revision
}

fn insert_sql(worker: &Worker, path: &Path, revision: u64, sql: &str, expected: &[u8]) {
    let before = std::fs::read(path).unwrap();
    let (target, _) = inspect(worker, revision, sql);
    let (stale, _) = inspect(worker, revision, sql);
    let Response::QueryInsertReviewed { review, capability } = request(
        worker,
        Command::ReviewQueryInsert {
            target,
            parameters: Vec::new(),
        },
    )
    .unwrap() else {
        panic!("manual indexed SQL INSERT review");
    };
    assert_eq!(review.inserted_rows, 1);
    assert_image(&std::fs::read(path).unwrap(), &before);
    assert!(!crate::journal_path(path).exists());
    let saves = SaveWorker::new(|| {});
    let id = saves
        .start_query_insert(capability, crate::journal_path(path))
        .unwrap();
    let report = terminal(&saves);
    assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.matched_rows.is_none() && receipt.changed_rows == Some(1)
        && receipt.schema_change.is_none()));
    assert!(saves.acknowledge(id));
    assert_image(&std::fs::read(path).unwrap(), expected);
    request(worker, open(path, "")).unwrap();
    assert!(matches!(
        request(
            worker,
            Command::ReviewQueryInsert {
                target: stale,
                parameters: Vec::new()
            }
        ),
        Err(Error::StaleQuery)
    ));
    assert!(!crate::journal_path(path).exists());
}

#[test]
fn manual_indexed_insert_after_created_and_updated_source_retains_original_presence() {
    for descending in [false, true] {
        for value in [None, Some(77)] {
            let case = inserting::source(descending);
            let source = &case.updated.creation;
            let (_directory, path, worker, revision) = opened(&source.before);
            let revision = create(&worker, &path, revision, source);
            let sql = format!(
                "UPDATE [{}] SET [{}]=CLng(Nz([{}],0)+1)",
                source.table, source.column, source.column
            );
            let revision = update(&worker, &path, revision, &sql, &case.updated.expected);
            let expected = inserting::after(&case.updated, &[value]);
            let sql = format!(
                "INSERT INTO [{}] ([{}]) VALUES ({})",
                source.table,
                source.column,
                value.map_or_else(|| "NULL".into(), |value| value.to_string())
            );
            insert_sql(&worker, &path, revision, &sql, &expected);
        }

        let case = inserting::source(descending);
        let source = &case.updated.creation;
        let (_directory, path, worker, revision) = opened(&source.before);
        let revision = create(&worker, &path, revision, source);
        let sql = format!(
            "UPDATE [{}] SET [{}]=CLng(Nz([{}],0)+1)",
            source.table, source.column, source.column
        );
        let revision = update(&worker, &path, revision, &sql, &case.updated.expected);
        let Response::InsertDraft(draft) = request(
            &worker,
            Command::BeginInsert {
                target: InsertTarget {
                    document_revision: revision,
                    table: source.table.into(),
                },
            },
        )
        .unwrap() else {
            panic!("real row-add draft");
        };
        assert_eq!(draft.fields.len(), 1);
        assert!(!draft.fields[0].auto_number);
        let Response::InsertReviewed { review, capability } = request(
            &worker,
            Command::ReviewInsert {
                draft_id: draft.id,
                inputs: vec![InsertInput::Omitted],
            },
        )
        .unwrap() else {
            panic!("omitted manual indexed field review");
        };
        assert!(matches!(review.inputs.as_slice(), [InsertInput::Omitted]));
        assert_image(&std::fs::read(&path).unwrap(), &case.updated.expected);
        let saves = SaveWorker::new(|| {});
        let id = saves
            .start_insert(capability, crate::journal_path(&path))
            .unwrap();
        let report = terminal(&saves);
        assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
            if receipt.matched_rows.is_none() && receipt.changed_rows == Some(1)
            && receipt.schema_change.is_none()));
        assert!(saves.acknowledge(id));
        let expected = inserting::after(&case.updated, &[None]);
        assert_image(&std::fs::read(&path).unwrap(), &expected);
        request(&worker, open(&path, "")).unwrap();
        let Response::Data { rows, .. } = request(
            &worker,
            Command::Page {
                table: source.table.into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("fresh manual insertion rows");
        };
        assert_eq!(rows.len(), source.rows.len() + 1);
        assert_eq!(rows.last().unwrap()[0].text, "NULL");
        assert!(!crate::journal_path(&path).exists());
    }
}

#[test]
fn manual_indexed_insert_preserves_compressed_guard_currency_and_unicode_companions() {
    for descending in [false, true] {
        let case = inserting::compressed::source(descending);
        let source = &case.creation;
        let (_directory, path, worker, revision) = opened(&source.before);
        let sql = format!(
            "CREATE INDEX [{}] ON [{}] ([{}] {})",
            source.index_name,
            source.table,
            source.column,
            if descending { "DESC" } else { "ASC" }
        );
        let revision = create_definition(&worker, &path, revision, &sql, &source.expected);
        let sql = format!("UPDATE [{}] SET Guard=5", source.table);
        let revision = update(&worker, &path, revision, &sql, &case.updated);
        let sql = format!(
            "INSERT INTO [{}] (KeyValue,Guard,Amount,Label) VALUES (99,NULL,CCur(1.2345),'新🦀')",
            source.table
        );
        insert_sql(&worker, &path, revision, &sql, &case.expected);
        let Response::Data { rows, .. } = request(
            &worker,
            Command::Page {
                table: source.table.into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("fresh protected scalar rows");
        };
        let row = rows.last().unwrap();
        assert_eq!(row[0].text, "99");
        assert_eq!(row[1].text, "NULL");
        assert_eq!(row[3].text, "新🦀");
    }
}
