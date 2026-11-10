//! Existing row/index/generation bytes survive a separately admitted schema rename.
use super::*;
use tablecraft_builders::table::field_rename::FieldRenameNameProfile;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::inserting,
    saved_field_rename::fitting::{self, SourceCase},
};

#[test]
fn populated_field_rename_inspection_review_save_and_reopen_preserve_the_actual_row() {
    for replacement in ["TaxAmount", "X"] {
        let [empty, renamed] = fitting::pair(SourceCase::NumberFirst, replacement);
        let before = inserting::first_number_row(&empty, &[(0, Some(1)), (1, Some(42))]);
        let after = inserting::first_number_row(&renamed, &[(0, Some(1)), (1, Some(42))]);
        let (directory, path, worker, revision) = opened(&before);
        let Response::Data { rows, .. } = request(
            &worker,
            Command::Page {
                table: "Table1".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("the original populated source must already be readable");
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0].text, "1");
        assert_eq!(rows[0][1].text, "42");
        let Response::FieldRenameSourceInspected { inspection } = request(
            &worker,
            Command::InspectFieldRenameSource {
                document_revision: revision,
                table: "Table1".into(),
                column: "Score".into(),
                native_number: 1,
            },
        )
        .unwrap() else {
            panic!("expected actual populated-source inspection");
        };
        assert!(inspection.source_eligible, "{inspection:?}");
        assert_eq!(
            inspection.name_profile,
            Some(FieldRenameNameProfile::FittedAscii { original_units: 5 })
        );
        let (_, capability) = review(&worker, revision, "Score", 1, replacement).unwrap();
        let witness = capability.reopen_witness();
        assert_eq!(witness.expected_bytes(), after);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let bundle = directory.path().join("populated-rename.journal");
        assert!(!bundle.exists());
        let saves = SaveWorker::new(|| {});
        let id = saves
            .start_field_rename(capability, bundle.clone())
            .unwrap();
        let report = terminal(&saves);
        let SaveOutcome::Saved { receipt } = report.outcome else {
            panic!("expected qualified populated schema save: {report:?}");
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
        let Response::Data {
            columns,
            rows,
            has_more,
            ..
        } = request(
            &worker,
            Command::Page {
                table: "Table1".into(),
                offset: 0,
            },
        )
        .unwrap()
        else {
            panic!("the checked full replacement must retain the original row");
        };
        assert_eq!(columns[1].name, replacement);
        assert_eq!(columns[1].native_number, Some(1));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0].text, "1");
        assert_eq!(rows[0][1].text, "42");
        assert!(!has_more);
    }
}
