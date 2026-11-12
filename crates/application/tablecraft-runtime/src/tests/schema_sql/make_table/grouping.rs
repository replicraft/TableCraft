//! Real issued grouped SELECT INTO requests stage once and publish one complete table.
use super::{assert_image, fields, inspect, populated};
use crate::tests::{request, stored::opened, terminal};
use crate::{Command, Error, ParameterInput, Response, SaveOutcome, SaveSchemaChange, SaveWorker};
use std::fs;
use tablecraft_access::editing::sql::NativeMakeTableError;
use tablecraft_expressions::EvalError;
use tablecraft_query::{MakeTableError, QueryError};

const SQL: &str = "PARAMETERS [Min] LONG; SELECT CLng(Count(*)) AS CopiedScore INTO Archive FROM Items AS i GROUP BY i.CopiedScore HAVING Count(*)>=[Min] ORDER BY i.CopiedScore";

#[test]
fn grouped_having_inputs_preserve_null_groups_and_empty_destinations_through_actual_save() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let before = populated::numbers(
        &seed,
        "Items",
        &[None, None, Some(1), Some(1), Some(2), Some(2), Some(2)],
    );
    for (text, is_null, selected) in [
        ("2", false, vec![Some(2), Some(2), Some(3)]),
        ("3", false, vec![Some(3)]),
        ("4", false, Vec::new()),
        ("not parsed for NULL", true, Vec::new()),
    ] {
        let expected = populated::numbers(&before, "Archive", &selected);
        let (_directory, path, worker, revision) = opened(&before);
        let target = inspect(&worker, revision, SQL);
        let Response::TableCreationReviewed { capability } = request(
            &worker,
            Command::ReviewParameterizedSchemaSql {
                target,
                parameters: vec![ParameterInput {
                    name: "Min".into(),
                    kind: "int32".into(),
                    text: text.into(),
                    is_null,
                }],
            },
        )
        .unwrap() else {
            panic!("issued grouped native creation");
        };
        let witness = capability.reopen_witness();
        assert_eq!(witness.inserted_rows(), Some(selected.len()));
        assert_eq!(witness.column_count(), 1);
        assert_image(witness.expected_bytes(), &expected);
        assert_image(&fs::read(&path).unwrap(), &before);
        let bundle = crate::journal_path(&path);
        assert!(!bundle.exists());

        let saves = SaveWorker::new(|| {});
        // NO_CLONE: The save worker consumes its namespace path; this test keeps
        // the same independently owned path to verify cleanup after acknowledgment.
        let id = saves
            .start_table_creation(capability, bundle.clone())
            .unwrap();
        let report = terminal(&saves);
        assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
            if receipt.matched_rows.is_none()
            && receipt.changed_rows == Some(selected.len())
            && receipt.schema_change == Some(SaveSchemaChange::TableAdded { column_count: 1 })));
        assert!(saves.acknowledge(id));
        assert_image(&fs::read(&path).unwrap(), &expected);
        let Response::Opened {
            document_revision: fresh,
            tables,
            ..
        } = request(&worker, Command::OpenTableCreated { witness }).unwrap()
        else {
            panic!("guarded grouped destination reopen");
        };
        assert!(fresh > revision);
        assert!(tables.iter().any(|table| table.name == "Archive"));
        assert!(!bundle.exists());
        assert_image(&fs::read(path).unwrap(), &expected);
    }
}

#[test]
fn later_and_top_omitted_group_failures_do_not_issue_native_capability_or_namespace() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let before = populated::numbers(&seed, "Items", &[Some(1), Some(1), Some(2)]);
    let (_directory, path, worker, revision) = opened(&before);
    for sql in [
        "SELECT TOP 1 CLng(Count(*)) AS CopiedScore INTO Archive FROM Items GROUP BY CopiedScore HAVING 1/(Count(*)-1)>0 ORDER BY CopiedScore",
        "SELECT CLng(Sum(1/(CopiedScore-2))) AS CopiedScore INTO Archive FROM Items GROUP BY CopiedScore",
    ] {
        let target = inspect(&worker, revision, sql);
        assert!(matches!(
            request(&worker, Command::ReviewSchemaSql { target }),
            Err(Error::NativeMakeTable(NativeMakeTableError::Query(
                MakeTableError::Query(QueryError::Evaluation(EvalError::DivisionByZero))
            ))),
        ));
        assert_image(&fs::read(&path).unwrap(), &before);
        assert!(!crate::journal_path(&path).exists());
    }
}
