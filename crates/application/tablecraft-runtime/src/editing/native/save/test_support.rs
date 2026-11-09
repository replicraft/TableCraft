//! Actual public-worker cell review shared by native actor boundary tests.
use crate::{Command, EditInput, EditTarget, Response, ReviewedEdit, Worker};
use std::path::Path;

pub(super) fn review_source(source: &Path) -> (Worker, ReviewedEdit) {
    let reader = Worker::new(|| {});
    let Response::Opened {
        document_revision, ..
    } = crate::tests::request(
        &reader,
        Command::Open {
            path: source.to_owned(),
            password: String::new(),
        },
    )
    .unwrap()
    else {
        panic!("expected original source");
    };
    let capability = review(&reader, document_revision);
    (reader, capability)
}

pub(super) fn review(reader: &Worker, document_revision: u64) -> ReviewedEdit {
    let Response::EditDraft(draft) = crate::tests::request(
        reader,
        Command::BeginEdit {
            target: EditTarget {
                document_revision,
                table: "Items".into(),
                column: "Score".into(),
                parent_ordinal: 1,
            },
        },
    )
    .unwrap() else {
        panic!("expected actual cell draft");
    };
    let Response::EditReviewed { capability, .. } = crate::tests::request(
        reader,
        Command::ReviewEdit {
            draft_id: draft.id,
            input: EditInput {
                text: "42".into(),
                is_null: false,
            },
        },
    )
    .unwrap() else {
        panic!("expected actual consuming review");
    };
    capability
}
