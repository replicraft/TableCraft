use super::{AccessEngineeringEditor, ScalarUpdateMode};
use crate::{
    NativeEditor,
    engineering::{UpdateMode, sql},
};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, IndexCheckStatus, InspectError, NativeImage, WriteBlocker, check_table,
    check_table_constraints, read_table,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_db::{CellUpdate, EditableTable, UpdateProfile};
use tablecraft_expressions::EvalError;
use tablecraft_model::CatalogScope;
use tablecraft_query::{QueryError, UpdateError};
use tablecraft_sql::{ParseLimits, parse_query};
use tablecraft_testkit::access as support;

#[path = "../../../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
pub(in crate::engineering) mod fixture;

mod batch;
mod metadata_default;
mod scalar_functions;
mod source;
use source::source;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn rows(bytes: &[u8]) -> Vec<Vec<Value>> {
    read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .into_parts()
    .2
}

#[test]
fn qualify_sql_engineering_source_before_new_editor_api() {
    let bytes = source();
    let checked = check_table(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 4);
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let constraints = check_table_constraints(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(constraints.rows(), 4);
    assert_eq!(
        rows(&bytes)[2],
        vec![
            Value::Int32(3),
            Value::Text("A".to_owned()),
            Value::Int32(30),
            Value::Text("Z".to_owned())
        ]
    );
}

#[test]
fn selected_text_rows_and_small_int32_literals_finish_one_complete_copy() {
    let before = source();
    let query = parse_query(
        "UPDATE Items AS i SET i.Label=i.Label & '-edited' WHERE i.Id IN (1,3)",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = sql::preview_update(
        image(&before),
        query.query(),
        ScalarUpdateMode::Variable { index_growth: true },
        &mut budget(),
        None,
        &|| false,
    )
    .unwrap();
    assert_eq!(plan.matched_rows(), 2);
    let copy = plan.into_backend();
    assert_eq!(copy.before_bytes(), before);
    assert_eq!(copy.affected_rows(), 2);
    let after = rows(copy.candidate_bytes());
    assert_eq!(
        after.iter().map(|row| row[1].clone()).collect::<Vec<_>>(),
        vec![
            Value::Text("A-edited".to_owned()),
            Value::Text("B".to_owned()),
            Value::Text("A-edited".to_owned()),
            Value::Text("C".to_owned())
        ]
    );
    assert_eq!(
        after.iter().map(|row| row[3].clone()).collect::<Vec<_>>(),
        rows(&before)
            .iter()
            .map(|row| row[3].clone())
            .collect::<Vec<_>>()
    );

    for (sql_text, expected) in [
        ("UPDATE Items SET Score=11 WHERE Id>=2", Value::Int32(11)),
        ("UPDATE Items SET Score=NULL WHERE Id>=2", Value::Null),
    ] {
        let query = parse_query(sql_text, ParseLimits::default()).unwrap();
        let plan = sql::preview_update(
            image(&before),
            query.query(),
            ScalarUpdateMode::Fixed(UpdateMode::Fitted),
            &mut budget(),
            None,
            &|| false,
        )
        .unwrap();
        assert_eq!(plan.matched_rows(), 3);
        let copy = plan.into_backend();
        assert_eq!(copy.before_bytes(), before);
        let after = rows(copy.candidate_bytes());
        assert_eq!(after[0][2], Value::Int32(10));
        assert!(after[1..].iter().all(|row| row[2] == expected));
    }
}

#[test]
fn sql_null_is_distinct_from_literal_null_and_normal_ace_stays_unavailable() {
    let before = source();
    for (literal, expected) in [
        ("NULL", Value::Null),
        ("'null'", Value::Text("null".to_owned())),
    ] {
        let text = format!("UPDATE Items SET Label={literal} WHERE Id=2");
        let query = parse_query(&text, ParseLimits::default()).unwrap();
        let plan = sql::preview_update(
            image(&before),
            query.query(),
            ScalarUpdateMode::Variable {
                index_growth: false,
            },
            &mut budget(),
            None,
            &|| false,
        )
        .unwrap();
        assert_eq!(plan.matched_rows(), 1);
        assert_eq!(rows(plan.into_backend().candidate_bytes())[1][1], expected);
    }
    let normal = NativeEditor::new(image(&before), "Items", &mut budget(), &|| false).unwrap();
    assert_eq!(normal.update_profile(), UpdateProfile::Unavailable);
}

#[test]
fn stale_snapshot_handles_wrong_carriers_and_modes_refuse_without_conversion() {
    let before = source();
    let first = AccessEngineeringEditor::new(
        image(&before),
        "Items",
        ScalarUpdateMode::Variable {
            index_growth: false,
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let stale = first.snapshot().row_handle(0).unwrap();
    let second = AccessEngineeringEditor::new(
        image(&before),
        "Items",
        ScalarUpdateMode::Variable {
            index_growth: false,
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        second.plan_updates(
            "Label",
            &[CellUpdate::new(
                stale,
                Value::Text("wrong snapshot".to_owned())
            )],
            &mut budget(),
            &|| false
        ),
        Err(crate::NativeEditError::Snapshot(
            tablecraft_db::SnapshotError::StaleRow
        ))
    ));
    let query = parse_query(
        "UPDATE Items SET Label=11 WHERE Id=1",
        ParseLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        sql::preview_update(
            image(&before),
            query.query(),
            ScalarUpdateMode::Variable {
                index_growth: false
            },
            &mut budget(),
            None,
            &|| false
        ),
        Err(UpdateError::Query(QueryError::Unsupported))
    ));
    let query = parse_query(
        "UPDATE Items SET Label='wrong mode' WHERE Id<0",
        ParseLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        sql::preview_update(
            image(&before),
            query.query(),
            ScalarUpdateMode::LongValue,
            &mut budget(),
            None,
            &|| false
        ),
        Err(UpdateError::Backend(crate::NativeEditError::Edit(
            EditError::Unsupported(WriteBlocker::Column)
        )))
    ));
}

#[test]
fn late_and_eager_query_errors_return_no_engineering_copy() {
    let before = source();
    for text in [
        "UPDATE Items SET Score=10 \\ (4-Id)",
        "UPDATE Items SET Score=11 WHERE Id<0 AND (1 \\ 0)=0",
    ] {
        let query = parse_query(text, ParseLimits::default()).unwrap();
        assert!(matches!(
            sql::preview_update(
                image(&before),
                query.query(),
                ScalarUpdateMode::Fixed(UpdateMode::Fitted),
                &mut budget(),
                None,
                &|| false
            ),
            Err(UpdateError::Query(QueryError::Evaluation(
                EvalError::DivisionByZero
            )))
        ));
    }
    let query = parse_query("UPDATE Items SET Label='cancelled'", ParseLimits::default()).unwrap();
    assert!(matches!(
        sql::preview_update(
            image(&before),
            query.query(),
            ScalarUpdateMode::Variable {
                index_growth: false
            },
            &mut budget(),
            None,
            &|| true
        ),
        Err(UpdateError::Backend(crate::NativeEditError::Inspect(
            InspectError::Cancelled
        )))
    ));
}
