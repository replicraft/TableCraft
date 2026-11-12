//! Actual issued parameter review stages once and saves the complete native relation.
use super::{assert_image, fields, inspect, populated, review};
use crate::tests::{request, stored::opened, terminal};
use crate::{Command, Error, ParameterInput, Response, SaveOutcome, SaveSchemaChange, SaveWorker};
use std::{fs, sync::Arc};

fn minimum(text: &str, is_null: bool) -> ParameterInput {
    ParameterInput {
        name: "Min".into(),
        kind: "int32".into(),
        text: text.into(),
        is_null,
    }
}

#[test]
fn declared_selection_inputs_save_exact_rows_and_explicit_null_selects_no_rows() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let before = populated::numbers(&seed, "Items", &[None, Some(1), Some(5), Some(9)]);
    let sql = "PARAMETERS [Min] LONG; SELECT i.CopiedScore INTO Archive FROM Items AS i WHERE i.CopiedScore >= [Min] ORDER BY i.CopiedScore";
    for (input, selected) in [
        (minimum("5", false), vec![Some(5), Some(9)]),
        (minimum("not parsed for NULL", true), Vec::new()),
    ] {
        let (_directory, path, worker, revision) = opened(&before);
        let target = inspect(&worker, revision, sql);
        assert!(matches!(
            review(&worker, &target),
            Err(Error::SchemaSqlUnsupported)
        ));
        let Response::TableCreationReviewed { capability } = request(
            &worker,
            Command::ReviewParameterizedSchemaSql {
                target: Arc::clone(&target),
                parameters: vec![input],
            },
        )
        .unwrap() else {
            panic!("expected actual parameterized creation review");
        };
        let expected = populated::numbers(&before, "Archive", &selected);
        let witness = capability.reopen_witness();
        assert_eq!(witness.inserted_rows(), Some(selected.len()));
        assert_eq!(witness.column_count(), 1);
        assert_image(witness.expected_bytes(), &expected);
        assert_image(&fs::read(&path).unwrap(), &before);
        let bundle = crate::journal_path(&path);
        assert!(!bundle.exists());
        let saves = SaveWorker::new(|| {});
        let id = saves
            .start_table_creation(capability, bundle.clone())
            .unwrap();
        let report = terminal(&saves);
        assert!(matches!(
            report.outcome,
            SaveOutcome::Saved { receipt }
                if receipt.schema_change == Some(SaveSchemaChange::TableAdded { column_count: 1 })
                    && receipt.matched_rows.is_none()
                    && receipt.changed_rows == Some(selected.len())
        ));
        assert!(saves.acknowledge(id));
        assert_image(&fs::read(&path).unwrap(), &expected);
        let Response::Opened {
            document_revision: fresh,
            ..
        } = request(&worker, Command::OpenTableCreated { witness }).unwrap()
        else {
            panic!("expected exact guarded reopen");
        };
        assert!(fresh > revision);
        // Inputs were consumed during preparation. Replaying the old issued
        // target after publication must fail before any new literal parsing.
        assert!(matches!(
            request(
                &worker,
                Command::ReviewParameterizedSchemaSql {
                    target,
                    parameters: Vec::new(),
                },
            ),
            Err(Error::StaleQuery)
        ));
        assert!(!bundle.exists());
        assert_image(&fs::read(path).unwrap(), &expected);
    }
}

#[test]
fn missing_extra_duplicate_and_wrong_carriers_never_issue_a_creation_capability() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let before = populated::numbers(&seed, "Items", &[Some(1)]);
    let (_directory, path, worker, revision) = opened(&before);
    let target = inspect(
        &worker,
        revision,
        "PARAMETERS [Min] LONG; SELECT CopiedScore INTO Archive FROM Items WHERE CopiedScore >= [Min]",
    );
    let mut unexpected = minimum("5", false);
    unexpected.name = "Other".into();
    let mut wrong_carrier = minimum("5", false);
    wrong_carrier.kind = "text".into();
    for parameters in [
        Vec::new(),
        vec![unexpected],
        vec![minimum("1", false), minimum("5", false)],
        vec![wrong_carrier],
    ] {
        assert!(matches!(
            request(
                &worker,
                Command::ReviewParameterizedSchemaSql {
                    target: Arc::clone(&target),
                    parameters,
                },
            ),
            Err(Error::ParameterInput)
        ));
        assert_image(&fs::read(&path).unwrap(), &before);
        assert!(!crate::journal_path(&path).exists());
    }
    let (_other_directory, other_path, other_worker, _) = opened(&before);
    assert!(matches!(
        request(
            &other_worker,
            Command::ReviewParameterizedSchemaSql {
                target,
                parameters: Vec::new(),
            },
        ),
        Err(Error::StaleQuery)
    ));
    assert_image(&fs::read(other_path).unwrap(), &before);
}

#[test]
fn late_parameterized_expression_errors_and_cancelled_reviews_have_no_effects() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let before = populated::numbers(&seed, "Items", &[Some(1), Some(5), Some(9)]);
    let (_directory, path, worker, revision) = opened(&before);
    let target = inspect(
        &worker,
        revision,
        "PARAMETERS [Min] LONG; SELECT CopiedScore INTO Archive FROM Items WHERE 1/([Min]-CopiedScore)>0",
    );
    assert!(matches!(
        request(
            &worker,
            Command::ReviewParameterizedSchemaSql {
                target,
                parameters: vec![minimum("9", false)],
            },
        ),
        Err(Error::NativeMakeTable(
            tablecraft_access::editing::sql::NativeMakeTableError::Query(
                tablecraft_query::MakeTableError::Query(tablecraft_query::QueryError::Evaluation(
                    tablecraft_expressions::EvalError::DivisionByZero
                ))
            )
        ))
    ));
    assert_image(&fs::read(&path).unwrap(), &before);
    assert!(!crate::journal_path(&path).exists());
    let (mut document, _) =
        crate::document::Document::open(&path, &mut String::new(), 7, &|| false).unwrap();
    let Response::QueryParameters { target, .. } = document.inspect_query(
        7,
        "PARAMETERS [Min] LONG; SELECT CopiedScore INTO Archive FROM Items WHERE CopiedScore >= [Min]",
        &|| false,
    ).unwrap() else {
        panic!("expected issued parameter inspection");
    };
    assert!(matches!(
        document.review_parameterized_schema_sql(
            &target,
            &[minimum("5", false)],
            &crate::query::context::LocalClock,
            &|| true,
        ),
        Err(Error::Cancelled)
    ));
    assert_image(&fs::read(path).unwrap(), &before);
}
