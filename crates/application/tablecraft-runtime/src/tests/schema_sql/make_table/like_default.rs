//! SQL View's application default reaches real SELECT INTO without native retagging.
use super::{assert_image, fields, populated};
use crate::tests::schema_sql::{inspect, review};
use crate::tests::{request, stored::opened, terminal};
use crate::{Command, Error, Response, SaveOutcome, SaveSchemaChange, SaveWorker};

#[test]
fn absent_native_mode_uses_sql_view_ansi89_through_make_table_review_save_ack_reopen() {
    for (pattern, selected) in [("a*", vec![Some(5)]), ("a%", vec![])] {
        let seed = fields::source(fields::Case::FirstNumber).bytes;
        let before = populated::numbers(&seed, "Items", &[None, Some(2), Some(5)]);
        let expected = populated::numbers(&before, "Archive", &selected);
        let (_directory, path, worker, revision) = opened(&before);
        let sql = format!(
            "SELECT CopiedScore INTO Archive FROM Items WHERE 'Apple' LIKE '{pattern}' AND CopiedScore>=5 ORDER BY CopiedScore"
        );
        let target = inspect(&worker, revision, &sql);
        let Response::TableCreationReviewed { capability } = review(&worker, &target).unwrap()
        else {
            panic!("SQL View default make-table review");
        };
        let witness = capability.reopen_witness();
        assert_eq!(witness.inserted_rows(), Some(selected.len()));
        assert_image(witness.expected_bytes(), &expected);
        assert_image(&std::fs::read(&path).unwrap(), &before);
        assert!(!crate::journal_path(&path).exists());
        let saves = SaveWorker::new(|| {});
        let id = saves
            .start_table_creation(capability, crate::journal_path(&path))
            .unwrap();
        let report = terminal(&saves);
        match report.outcome {
            SaveOutcome::Saved { receipt } => {
                assert_eq!(receipt.changed_rows, Some(selected.len()));
                assert_eq!(
                    receipt.schema_change,
                    Some(SaveSchemaChange::TableAdded { column_count: 1 })
                );
            }
            outcome => panic!("make-table save failed: {outcome:?}"),
        }
        assert!(saves.acknowledge(id));
        assert_image(&std::fs::read(&path).unwrap(), &expected);
        assert!(matches!(
            request(&worker, Command::OpenTableCreated { witness }),
            Ok(Response::Opened { .. })
        ));
        assert!(matches!(review(&worker, &target), Err(Error::StaleQuery)));
        let Response::Data { rows, .. } = request(
            &worker,
            Command::Page {
                table: "Archive".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("fresh Archive rows");
        };
        assert_eq!(rows.len(), selected.len());
        if !selected.is_empty() {
            assert_eq!(rows[0][0].text, "5");
        }
        assert_image(&std::fs::read(&path).unwrap(), &expected);
        assert!(!crate::journal_path(&path).exists());
    }
}
