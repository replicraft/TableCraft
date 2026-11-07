use super::{opened, request, review, source, value};
use crate::{Command, Error, InsertInput, Response, document::Document, journal_path};
use std::sync::Arc;

#[test]
fn insert_original_witness_reopens_only_complete_original_bytes_without_replaying_insert() {
    let original = source();
    let (_directory, path, worker, revision) = opened(&original);
    let (_, capability) = review(
        &worker,
        revision,
        vec![value("4", false), InsertInput::Omitted],
    );
    let witness = capability.retry_witness();
    assert_eq!(witness.source_path(), path);
    assert_eq!(witness.target().table, "Items");
    assert_eq!(witness.target().document_revision, revision);
    let Response::Opened {
        document_revision, ..
    } = request(
        &worker,
        Command::OpenInsertOriginal {
            witness: Arc::clone(&witness),
        },
    )
    .unwrap()
    else {
        panic!("expected a fresh read-only original snapshot");
    };
    assert_ne!(document_revision, revision);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!journal_path(&path).exists());
    let Response::Data { rows, .. } = request(
        &worker,
        Command::Page {
            table: "Items".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected unchanged source rows");
    };
    assert_eq!(rows.len(), 1);
    let mut changed = original;
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();
    assert!(matches!(
        request(&worker, Command::OpenInsertOriginal { witness }),
        Err(Error::InsertOriginalSourceChanged)
    ));
    assert!(matches!(
        request(
            &worker,
            Command::Page {
                table: "Items".into(),
                offset: 0
            }
        ),
        Err(Error::NoDatabase)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), changed);
    assert!(!journal_path(&path).exists());
}

#[test]
fn guarded_insert_original_open_retains_cancellation_and_snapshot_bounds() {
    let (_directory, path, worker, revision) = opened(&source());
    let (_, capability) = review(
        &worker,
        revision,
        vec![value("4", false), InsertInput::Omitted],
    );
    let witness = capability.retry_witness();
    assert!(matches!(
        Document::open_insert_original(&witness, revision + 1, &|| true),
        Err(Error::Cancelled)
    ));
    let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.set_len(64 * 1024 * 1024 + 1).unwrap();
    drop(file);
    assert!(matches!(
        request(&worker, Command::OpenInsertOriginal { witness }),
        Err(Error::SnapshotLimit)
    ));
    assert!(!journal_path(&path).exists());
}
