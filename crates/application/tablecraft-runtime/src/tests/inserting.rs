//! Public-worker INSERT review and the same nonreplaceable native save owner.
use super::{open, request, stored::opened, terminal};
use crate::{
    Command, EditError, EditInput, Error, InsertInput, InsertReview, InsertTarget, Response,
    ReviewedInsert, SaveFailureKind, SaveOutcome, SaveStartFailure, SaveWorker, Worker,
};
use tablecraft_testkit::fixtures::access::{canonical_creation, canonical_native17_creation};
use tablecraft_testkit::support::access::retained_target::assert_retained_target_observation;

fn source() -> Vec<u8> {
    canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::One).bytes,
    )
}

fn value(text: &str, is_null: bool) -> InsertInput {
    InsertInput::Value(EditInput {
        text: text.into(),
        is_null,
    })
}

fn review(
    worker: &Worker,
    revision: u64,
    inputs: Vec<InsertInput>,
) -> (InsertReview, ReviewedInsert) {
    let Response::InsertDraft(draft) = request(
        worker,
        Command::BeginInsert {
            target: InsertTarget {
                document_revision: revision,
                table: "Items".into(),
            },
        },
    )
    .unwrap() else {
        panic!("expected source-bound insertion fields");
    };
    assert_eq!(draft.fields.len(), 2);
    assert_eq!(draft.fields[0].name, "Id");
    assert_eq!(draft.fields[1].name, "Score");
    assert!(draft.fields.iter().all(|field| !field.auto_number));
    let Response::InsertReviewed { review, capability } = request(
        worker,
        Command::ReviewInsert {
            draft_id: draft.id,
            inputs,
        },
    )
    .unwrap() else {
        panic!("expected an ordinary admitted insertion review");
    };
    assert_eq!(review.inserted_rows, 1);
    (review, capability)
}

#[test]
fn reviewed_omission_and_explicit_null_preserve_intent_and_commit_one_row() {
    for omitted in [true, false] {
        let original = source();
        let (directory, path, worker, revision) = opened(&original);
        let second = if omitted {
            InsertInput::Omitted
        } else {
            value("retained NULL text", true)
        };
        let (review, capability) = review(&worker, revision, vec![value("4", false), second]);
        assert!(match &review.inputs[1] {
            InsertInput::Omitted => omitted,
            InsertInput::Value(input) =>
                !omitted && input.is_null && input.text == "retained NULL text",
        });
        assert_eq!(std::fs::read(&path).unwrap(), original);
        // No offline lease is held while the reviewed row waits for approval.
        tablecraft_access::editing::offline::OfflineWriteSession::open(
            &path,
            crate::controls::SCOPE,
            &mut crate::controls::budget(),
            &|| false,
        )
        .unwrap()
        .release()
        .unwrap();
        let saves = SaveWorker::new(|| {});
        let id = saves
            .start_insert(capability, directory.path().join("insert.journal"))
            .unwrap();
        let report = terminal(&saves);
        assert_eq!(report.id, id);
        let SaveOutcome::Saved { receipt } = report.outcome else {
            panic!("normal insert must publish its actual completed receipt");
        };
        assert_eq!(receipt.matched_rows, None);
        assert_eq!(receipt.changed_rows, Some(1));
        assert!(saves.acknowledge(id));
        assert!(!directory.path().join("insert.journal").exists());
        request(&worker, open(&path, "")).unwrap();
        let Response::Data { rows, .. } = request(
            &worker,
            Command::Page {
                table: "Items".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("inserted native row must reopen through the ordinary reader");
        };
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1][0].text, "4");
        assert!(matches!(
            rows[1][1].kind,
            crate::CellKind::Scalar { is_null: true, .. }
        ));
    }
}

#[test]
fn source_drift_is_refused_before_journal_and_its_owner_requires_release() {
    let original = source();
    let (directory, path, worker, revision) = opened(&original);
    let (_, capability) = review(
        &worker,
        revision,
        vec![value("4", false), InsertInput::Omitted],
    );
    let mut changed = original.clone();
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("insert.journal");
    let id = saves.start_insert(capability, bundle.clone()).unwrap();
    let report = terminal(&saves);
    assert_eq!(report.id, id);
    let SaveOutcome::Failed(failure) = report.outcome else {
        panic!("changed original bytes must not be inserted into");
    };
    assert_eq!(failure.kind, SaveFailureKind::SourceChanged);
    assert!(failure.can_release);
    assert!(!failure.can_recover);
    assert!(!saves.acknowledge(id));
    assert!(!bundle.exists());
    assert_retained_target_observation(&path, &changed);
    let released = saves.release(id).unwrap();
    let report = terminal(&saves);
    assert_eq!(report.id, released);
    assert!(matches!(
        report.outcome,
        SaveOutcome::Released {
            current_outcome: None,
            last_completed: None
        }
    ));
    assert!(saves.acknowledge(released));
    assert_eq!(std::fs::read(&path).unwrap(), changed);
}

#[test]
fn busy_worker_returns_the_unconsumed_insert_capability_and_stale_document_refuses_it() {
    let original = source();
    let (directory, path, worker, revision) = opened(&original);
    let (_, first) = review(
        &worker,
        revision,
        vec![value("4", false), InsertInput::Omitted],
    );
    let (_, second) = review(
        &worker,
        revision,
        vec![value("5", false), InsertInput::Omitted],
    );
    let saves = SaveWorker::new(|| {});
    let id = saves
        .start_insert(first, directory.path().join("first.journal"))
        .unwrap();
    let error = saves
        .start_insert(second, directory.path().join("second.journal"))
        .unwrap_err();
    assert_eq!(error.kind, SaveStartFailure::Busy);
    let second = error
        .into_reviewed()
        .expect("rejected start returns the insertion capability");
    assert_eq!(second.source_path(), path);
    assert_eq!(terminal(&saves).id, id);
    assert!(saves.acknowledge(id));
    request(&worker, open(&path, "")).unwrap();
    let error = saves
        .start_insert(second, directory.path().join("stale.journal"))
        .unwrap_err();
    assert_eq!(error.kind, SaveStartFailure::StaleDocument);
    assert!(error.into_reviewed().is_some());
    assert!(!directory.path().join("second.journal").exists());
    assert!(!directory.path().join("stale.journal").exists());
}

#[test]
fn insertion_review_rejects_stale_drafts_wrong_width_and_invalid_typed_values() {
    let original = source();
    let (_directory, path, worker, revision) = opened(&original);
    let target = InsertTarget {
        document_revision: revision,
        table: "Items".into(),
    };
    let Response::InsertDraft(first) = request(
        &worker,
        Command::BeginInsert {
            target: target.clone(),
        },
    )
    .unwrap() else {
        panic!("first insertion draft");
    };
    let Response::InsertDraft(second) = request(&worker, Command::BeginInsert { target }).unwrap()
    else {
        panic!("replacement insertion draft");
    };
    assert_ne!(first.id, second.id);
    assert!(matches!(
        request(
            &worker,
            Command::ReviewInsert {
                draft_id: first.id,
                inputs: vec![value("4", false), InsertInput::Omitted]
            }
        ),
        Err(Error::Edit(EditError::StaleDraft))
    ));
    assert!(matches!(
        request(
            &worker,
            Command::ReviewInsert {
                draft_id: second.id,
                inputs: vec![value("4", false)]
            }
        ),
        Err(Error::Edit(EditError::InvalidInput))
    ));
    assert!(
        request(
            &worker,
            Command::ReviewInsert {
                draft_id: second.id,
                inputs: vec![value("not a number", false), InsertInput::Omitted]
            }
        )
        .is_err()
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[cfg(feature = "ace16-create")]
mod initial_tables;

mod retry;
