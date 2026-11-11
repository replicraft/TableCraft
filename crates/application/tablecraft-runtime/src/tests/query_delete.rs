//! Reviewed SQL DELETE consumes typed inputs once, before confirmation and lease.
mod read_source;
use super::{open, query_parameters::inspect, request, stored::opened, terminal};
use crate::{
    Command, Error, ParameterInput, Response, ReviewedQueryDelete, SaveFailureKind, SaveOutcome,
    SaveWorker,
};
use tablecraft_testkit::{
    fixtures::access::{canonical_creation, canonical_native17_creation},
    support::access::retained_target::assert_retained_target_observation,
};

fn source() -> Vec<u8> {
    canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::Boundary).bytes,
    )
}

fn input(text: &str) -> ParameterInput {
    ParameterInput {
        name: "minimum".into(),
        kind: "int32".into(),
        text: text.into(),
        is_null: false,
    }
}

fn review(
    worker: &crate::Worker,
    revision: u64,
    parameters: Vec<ParameterInput>,
) -> (crate::QueryDeleteReview, ReviewedQueryDelete) {
    let (target, _) = inspect(
        worker,
        revision,
        "PARAMETERS minimum LONG; DELETE FROM Items WHERE Id>=minimum",
    );
    let Response::QueryDeleteReviewed { review, capability } =
        request(worker, Command::ReviewQueryDelete { target, parameters }).unwrap()
    else {
        panic!("expected source-bound SQL DELETE review");
    };
    assert_eq!(review.document_revision, revision);
    assert_eq!(review.table, "Items");
    (review, capability)
}

#[test]
fn typed_delete_review_holds_no_lease_saves_independent_after_and_reopens() {
    let original = source();
    let mut expected = original.clone();
    // Independent original fixture layout: TD7, DATA8, 64 ordinary live rows.
    for ordinal in [62, 63] {
        let slot = 8 * 4096 + 14 + 2 * ordinal;
        let value = u16::from_le_bytes(original[slot..slot + 2].try_into().unwrap());
        expected[slot..slot + 2].copy_from_slice(&(value | 0xc000).to_le_bytes());
    }
    expected[7 * 4096 + 16..7 * 4096 + 20].copy_from_slice(&62_u32.to_le_bytes());
    let (directory, path, worker, revision) = opened(&original);
    let (review, capability) = review(&worker, revision, vec![input("63")]);
    assert_eq!((review.matched_rows, review.deleted_rows), (2, 2));
    assert_eq!(capability.source_path(), path);
    assert_eq!(capability.document_revision(), revision);
    assert_eq!(capability.table(), "Items");
    assert_eq!(std::fs::read(&path).unwrap(), original);
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
    let bundle = directory.path().join("delete.journal");
    let id = saves
        .start_query_delete(capability, bundle.clone())
        .unwrap();
    let report = terminal(&saves);
    let SaveOutcome::Saved { receipt } = report.outcome else {
        panic!("expected completed SQL delete: {report:?}");
    };
    assert_eq!(
        (receipt.matched_rows, receipt.changed_rows),
        (Some(2), Some(2))
    );
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
        panic!("expected reopened deletion rows");
    };
    assert_eq!(rows.len(), 62);
    assert_eq!(rows[61][0].text, "62");
    assert_eq!(rows[61][1].text, "620");
}

#[test]
fn stale_delete_keeps_original_source_failure_and_retained_target_until_release() {
    let original = source();
    let (directory, path, worker, revision) = opened(&original);
    let (_, capability) = review(&worker, revision, vec![input("63")]);
    let mut changed = original;
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("stale-delete.journal");
    let id = saves
        .start_query_delete(capability, bundle.clone())
        .unwrap();
    let SaveOutcome::Failed(failure) = terminal(&saves).outcome else {
        panic!("expected stale source refusal");
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
    assert_eq!(std::fs::read(&path).unwrap(), changed);
}

#[test]
fn action_inputs_and_inspected_identity_refuse_before_unreadable_data() {
    let mut original = source();
    original[8 * 4096] = 0;
    let (_directory, path, worker, revision) = opened(&original);
    let (target, _) = inspect(
        &worker,
        revision,
        "PARAMETERS minimum LONG; DELETE FROM Items WHERE Id>=minimum",
    );
    for parameters in [
        Vec::new(),
        vec![ParameterInput {
            kind: "text".into(),
            ..input("63")
        }],
        vec![input("63"), input("63")],
    ] {
        assert!(matches!(
            request(
                &worker,
                Command::ReviewQueryDelete {
                    target: target.clone(),
                    parameters
                }
            ),
            Err(Error::ParameterInput)
        ));
    }
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryDelete {
                target: target.clone(),
                parameters: vec![input("not an integer")],
            }
        ),
        Err(Error::InvalidScalarInput)
    ));
    request(&worker, open(&path, "")).unwrap();
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryDelete {
                target,
                parameters: vec![input("63")]
            }
        ),
        Err(Error::StaleQuery)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!crate::journal_path(&path).exists());
}
