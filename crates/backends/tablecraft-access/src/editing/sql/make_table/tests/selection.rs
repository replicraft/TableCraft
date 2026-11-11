//! Row selection changes the materialized relation, never its native declaration.
use super::{SCOPE, budget, fields, image, prepare, raw, rows, syntax, text_source};
use crate::editing::sql::{NativeMakeTableError, prepare_native_make_table_with_native_comparison};
use std::{cell::RefCell, io::Cursor};
use tablecraft_ace::{TableSnapshotLimits, inspect_table_schema};
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{MakeTableError, QueryError};

fn source() -> Vec<u8> {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    raw::populated::numbers(
        &seed,
        "Items",
        &[None, Some(2), Some(5), Some(5), Some(9), Some(10)],
    )
}

#[test]
fn native_selection_retains_where_distinctrow_and_top_boundary_semantics() {
    let bytes = source();
    for (sql, expected) in [
        (
            "SELECT CopiedScore INTO Archive FROM Items WHERE CopiedScore >= 5 ORDER BY CopiedScore",
            vec![5, 5, 9, 10],
        ),
        (
            "SELECT DISTINCT CopiedScore INTO Archive FROM Items WHERE CopiedScore >= 5 ORDER BY CopiedScore",
            vec![5, 9, 10],
        ),
        (
            "SELECT DISTINCTROW CopiedScore INTO Archive FROM Items WHERE CopiedScore >= 5 ORDER BY CopiedScore",
            vec![5, 5, 9, 10],
        ),
        (
            "SELECT TOP 1 CopiedScore INTO Archive FROM Items WHERE CopiedScore >= 5 ORDER BY CopiedScore",
            vec![5, 5],
        ),
        (
            "SELECT TOP 50 PERCENT CopiedScore INTO Archive FROM Items WHERE CopiedScore >= 5 ORDER BY CopiedScore DESC",
            vec![10, 9],
        ),
        (
            "SELECT TOP 25 PERCENT CopiedScore INTO Archive FROM Items WHERE CopiedScore >= 2 ORDER BY CopiedScore DESC",
            vec![10, 9],
        ),
    ] {
        let prepared = prepare(&bytes, sql).unwrap();
        let values = expected
            .into_iter()
            .map(|value| vec![Value::Int32(value)])
            .collect::<Vec<_>>();
        assert_eq!(prepared.rows(), values, "{sql}");
        assert_eq!(prepared.definition().column_count(), 1);
        assert_eq!(prepared.definition().primary_position(), None);
    }
}

#[cfg(feature = "journal")]
#[test]
fn filtered_text_uses_the_actual_field_comparer_and_keeps_its_physical_width() {
    let (mut bytes, definition) = text_source();
    for input in [rows::Input::Text("B"), rows::Input::Text("a")] {
        bytes = rows::append(&bytes, definition, rows::Profile::DesktopText, input);
    }
    let prepared = prepare_native_make_table_with_native_comparison(
        image(&bytes),
        syntax("SELECT Name AS Label INTO Archive FROM Items WHERE Name < 'B' ORDER BY ID"),
        &RefCell::new(budget()),
        &RefCell::new(budget()),
        &mut budget(),
        &|| false,
        TableSnapshotLimits::default(),
    )
    .unwrap();
    assert_eq!(prepared.rows(), &[vec![Value::Text("a".into())]]);
    let plan = prepared.finish(&mut budget(), &|| false).unwrap();
    let schema = inspect_table_schema(
        &mut Cursor::new(plan.after_bytes()),
        SCOPE,
        "Archive",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(schema.columns()[0].name(), "Label");
    assert_eq!(schema.columns()[0].native_type(), 10);
    assert_eq!(schema.columns()[0].storage_length(), 14);
}

#[cfg(feature = "journal")]
#[test]
fn selected_and_zero_row_results_finish_as_exact_atomic_images() {
    let bytes = source();
    for (sql, selected) in [
        (
            "SELECT TOP 1 CopiedScore INTO Archive FROM Items WHERE CopiedScore >= 5 ORDER BY CopiedScore",
            vec![Some(5), Some(5)],
        ),
        (
            "SELECT CopiedScore INTO Archive FROM Items WHERE CopiedScore > 100",
            vec![],
        ),
    ] {
        let expected = raw::populated::numbers(&bytes, "Archive", &selected);
        let plan = prepare(&bytes, sql)
            .unwrap()
            .finish(&mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.row_count(), selected.len());
        assert_eq!(plan.before_bytes(), bytes);
        assert_eq!(plan.after_bytes().len(), expected.len());
        assert_eq!(
            plan.after_bytes()
                .iter()
                .zip(&expected)
                .position(|(actual, expected)| actual != expected),
            None,
            "first differing image byte for {sql}"
        );
    }
}

#[test]
fn omitted_rows_and_false_predicates_do_not_hide_eager_evaluation_errors() {
    let bytes = source();
    for sql in [
        "SELECT TOP 1 CopiedScore INTO Archive FROM Items WHERE 1/(10-CopiedScore)>0 ORDER BY CopiedScore",
        "SELECT CopiedScore INTO Archive FROM Items WHERE False AND 1/0=0",
    ] {
        assert!(
            matches!(
                prepare(&bytes, sql),
                Err(NativeMakeTableError::Query(MakeTableError::Query(
                    QueryError::Evaluation(EvalError::DivisionByZero)
                )))
            ),
            "{sql}"
        );
    }
}
