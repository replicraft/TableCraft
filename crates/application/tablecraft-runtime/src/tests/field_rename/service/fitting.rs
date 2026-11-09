//! Real runtime ownership for independently fitted names and complete AFTERs.
use super::*;
use tablecraft_builders::table::field_rename::FieldRenameNameProfile;
use tablecraft_testkit::fixtures::access::saved_field_rename::fitting::{self, SourceCase};

#[test]
fn fitted_number_and_text_reviews_save_and_reopen_the_independent_complete_image() {
    for case in [SourceCase::NumberFirst, SourceCase::TextNumberText] {
        for replacement in ["TaxAmount", "X"] {
            let original = fitting::old_name(case);
            let [before, after] = fitting::pair(case, replacement);
            let (directory, path, worker, revision) = opened(&before);
            let Response::FieldRenameSourceInspected { inspection } = request(
                &worker,
                Command::InspectFieldRenameSource {
                    document_revision: revision,
                    table: "Table1".into(),
                    column: original.into(),
                    native_number: 1,
                },
            )
            .unwrap() else {
                panic!("expected the actual original source profile");
            };
            assert!(inspection.source_eligible);
            assert_eq!(
                inspection.name_profile,
                Some(FieldRenameNameProfile::FittedAscii { original_units: 5 })
            );
            let (_, capability) = review(&worker, revision, original, 1, replacement).unwrap();
            let witness = capability.reopen_witness();
            assert_eq!(witness.expected_bytes(), after);
            assert_eq!(std::fs::read(&path).unwrap(), before);
            let bundle = directory.path().join("fitted.journal");
            assert!(!bundle.exists());
            let saves = SaveWorker::new(|| {});
            let id = saves
                .start_field_rename(capability, bundle.clone())
                .unwrap();
            let report = terminal(&saves);
            let SaveOutcome::Saved { receipt } = report.outcome else {
                panic!("expected a real fitted schema save: {report:?}");
            };
            assert_eq!(receipt.schema_change, Some(SaveSchemaChange::ColumnRenamed));
            assert_eq!((receipt.matched_rows, receipt.changed_rows), (None, None));
            assert!(saves.acknowledge(id));
            assert!(!bundle.exists());
            assert_eq!(std::fs::read(&path).unwrap(), after);
            assert!(matches!(
                request(&worker, Command::OpenFieldRenamed { witness }),
                Ok(Response::Opened { .. })
            ));
            let Response::Details { fields, .. } = request(
                &worker,
                Command::Describe {
                    table: "Table1".into(),
                },
            )
            .unwrap() else {
                panic!("expected fresh native schema after the exact witness read");
            };
            let field = fields
                .iter()
                .find(|field| field.name == replacement)
                .unwrap();
            assert_eq!(field.number, 1);
            assert!(fields.iter().all(|field| field.name != original));
        }
    }
}

#[test]
fn fitted_field_rename_images_admit_generated_row_review_without_losing_omission_intent() {
    let mut failures = Vec::new();
    for replacement in ["TaxAmount", "X"] {
        let [_, after] = fitting::pair(SourceCase::NumberFirst, replacement);
        let (_directory, path, worker, revision) = opened(&after);
        let Response::InsertDraft(draft) = request(
            &worker,
            Command::BeginInsert {
                target: crate::InsertTarget {
                    document_revision: revision,
                    table: "Table1".into(),
                },
            },
        )
        .unwrap() else {
            panic!("expected real insertion metadata after fitted field rename");
        };
        assert_eq!(draft.fields.len(), 2);
        assert_eq!(draft.fields[0].name, "ID");
        assert!(draft.fields[0].auto_number);
        assert_eq!(draft.fields[1].name, replacement);
        assert!(!draft.fields[1].auto_number);
        match request(
            &worker,
            Command::ReviewInsert {
                draft_id: draft.id,
                inputs: vec![crate::InsertInput::Omitted, crate::InsertInput::Omitted],
            },
        ) {
            Ok(Response::InsertReviewed { review, capability }) => {
                assert_eq!(review.inserted_rows, 1);
                assert!(
                    review
                        .inputs
                        .iter()
                        .all(|input| matches!(input, crate::InsertInput::Omitted))
                );
                assert_eq!(capability.source_path(), path);
            }
            Err(error) => failures.push(format!("{replacement}: {error:?}")),
            Ok(_) => panic!("a successful admission must return its actual consuming review"),
        }
        assert_eq!(std::fs::read(&path).unwrap(), after);
        assert!(!crate::journal_path(&path).exists());
    }
    assert!(
        failures.is_empty(),
        "post-rename row admission: {failures:?}"
    );
}
