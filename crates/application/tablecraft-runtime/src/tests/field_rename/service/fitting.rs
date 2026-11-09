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
