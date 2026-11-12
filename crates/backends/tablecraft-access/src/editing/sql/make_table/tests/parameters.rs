//! Declared literals affect selection once, without changing native field sizes.
use super::{budget, fields, image, raw, syntax};
use crate::editing::sql::{
    NativeMakeTableError, NativeMakeTableOptions, PreparedNativeMakeTable,
    prepare_parameterized_native_make_table_with_native_comparison,
};
use std::cell::RefCell;
use tablecraft_ace::TableSnapshotLimits;
use tablecraft_core::values::Value;
use tablecraft_query::{ExecutionOptions, MakeTableError, QueryError};

fn prepare(
    bytes: &[u8],
    sql: &str,
    parameters: &[(&str, Value)],
) -> Result<PreparedNativeMakeTable, NativeMakeTableError> {
    prepare_parameterized_native_make_table_with_native_comparison(
        image(bytes),
        syntax(sql),
        &RefCell::new(budget()),
        &RefCell::new(budget()),
        &mut budget(),
        NativeMakeTableOptions {
            parameters,
            execution: ExecutionOptions::default(),
            snapshot_limits: TableSnapshotLimits::default(),
        },
    )
}

#[cfg(feature = "journal")]
#[test]
fn declared_where_inputs_stage_exact_complete_rows_and_null_does_not_become_absent() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let before = raw::populated::numbers(&seed, "Items", &[None, Some(1), Some(5), Some(9)]);
    let sql = "PARAMETERS [Min] LONG; SELECT i.CopiedScore INTO Archive FROM Items AS i WHERE i.CopiedScore >= [Min] ORDER BY i.CopiedScore";
    for (value, selected) in [
        (Value::Int32(5), vec![Some(5), Some(9)]),
        (Value::Null, Vec::new()),
    ] {
        let parameters = [("Min", value)];
        let prepared = prepare(&before, sql, &parameters).unwrap();
        let expected_rows = selected
            .iter()
            .map(|value| vec![value.map_or(Value::Null, Value::Int32)])
            .collect::<Vec<_>>();
        assert_eq!(prepared.rows(), expected_rows);
        assert!(prepared.definition().is_manual());
        let expected = raw::populated::numbers(&before, "Archive", &selected);
        // The borrowed literals and SQL are no longer needed by the consuming
        // native operation; it uses only the captured declarations and rows.
        drop(parameters);
        let plan = prepared.finish(&mut budget(), &|| false).unwrap();
        assert_eq!(plan.after_bytes().len(), expected.len());
        assert_eq!(
            plan.after_bytes()
                .iter()
                .zip(&expected)
                .position(|(a, b)| a != b),
            None,
            "first differing complete image byte"
        );
    }
}

#[test]
fn binding_errors_and_old_strict_factory_do_not_produce_preparation() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let before = raw::populated::numbers(&seed, "Items", &[Some(1)]);
    let sql = "PARAMETERS [Min] LONG; SELECT CopiedScore INTO Archive FROM Items WHERE CopiedScore >= [Min]";
    for parameters in [
        Vec::new(),
        vec![("Other", Value::Int32(5))],
        vec![("Min", Value::Int32(5)), ("Min", Value::Int32(9))],
        vec![("Min", Value::Text("5".into()))],
    ] {
        assert!(prepare(&before, sql, &parameters).is_err());
    }
    assert!(matches!(
        super::prepare(&before, sql),
        Err(NativeMakeTableError::Shape)
    ));
    assert!(matches!(
        prepare(
            &before,
            "PARAMETERS [Min] LONG; SELECT CopiedScore INTO Archive FROM Items WHERE False AND 1/0=[Min]",
            &[("Min", Value::Int32(0))],
        ),
        Err(NativeMakeTableError::Query(MakeTableError::Query(
            QueryError::Evaluation(tablecraft_expressions::EvalError::DivisionByZero)
        )))
    ));
}
