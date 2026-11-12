//! Complete grouped staging retains native declarations and one atomic publication.
use super::prepare;
#[cfg(feature = "journal")]
use super::{budget, image, syntax};
use crate::editing::sql::NativeMakeTableError;
#[cfg(feature = "journal")]
use crate::editing::sql::{
    NativeMakeTableOptions, prepare_parameterized_native_make_table_with_native_comparison,
};
#[cfg(feature = "journal")]
use std::cell::RefCell;
#[cfg(feature = "journal")]
use tablecraft_ace::TableSnapshotLimits;
#[cfg(feature = "journal")]
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
#[cfg(feature = "journal")]
use tablecraft_query::ExecutionOptions;
use tablecraft_query::{MakeTableError, QueryError};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{fields, populated};

fn source() -> Vec<u8> {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    populated::numbers(
        &seed,
        "Items",
        &[None, None, Some(1), Some(1), Some(2), Some(2), Some(2)],
    )
}

#[cfg(feature = "journal")]
fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete native image length");
    assert_eq!(actual.iter().zip(expected).position(|(a, b)| a != b), None);
}

#[cfg(feature = "journal")]
#[test]
fn grouped_counts_null_keys_having_and_aggregate_sort_publish_complete_typed_images() {
    let before = source();
    for (sql, selected) in [
        (
            "SELECT CLng(Count(*)) AS CopiedScore INTO Archive FROM Items AS i GROUP BY i.CopiedScore HAVING Count(*)>=2 ORDER BY i.CopiedScore",
            vec![Some(2), Some(2), Some(3)],
        ),
        (
            "SELECT CLng(Count(i.CopiedScore)) AS CopiedScore INTO Archive FROM Items AS i GROUP BY i.CopiedScore ORDER BY i.CopiedScore",
            vec![Some(0), Some(2), Some(3)],
        ),
        (
            "SELECT CLng(Count(*)) AS CopiedScore INTO Archive FROM Items AS i GROUP BY i.CopiedScore HAVING Count(*)>=4 ORDER BY i.CopiedScore",
            Vec::new(),
        ),
        (
            "SELECT CLng(Count(*)) AS CopiedScore INTO Archive FROM Items AS i GROUP BY i.CopiedScore ORDER BY Count(*) DESC,i.CopiedScore",
            vec![Some(3), Some(2), Some(2)],
        ),
        (
            "SELECT CLng(Count(*)) AS CopiedScore INTO Archive FROM Items WHERE False",
            vec![Some(0)],
        ),
    ] {
        let prepared = prepare(&before, sql).unwrap_or_else(|error| panic!("{sql}: {error:?}"));
        assert_eq!(
            prepared.rows(),
            selected
                .iter()
                .map(|v| vec![v.map_or(Value::Null, Value::Int32)])
                .collect::<Vec<_>>()
        );
        let expected = populated::numbers(&before, "Archive", &selected);
        let plan = prepared.finish(&mut budget(), &|| false).unwrap();
        same(plan.before_bytes(), &before);
        same(plan.after_bytes(), &expected);
    }
}

#[cfg(feature = "journal")]
#[test]
fn uncast_count_uses_the_query_owner_static_carrier_even_with_no_source_rows() {
    let before = source();
    for (sql, values) in [
        (
            "SELECT Count(*) AS CopiedScore INTO Archive FROM Items",
            vec![Some(7)],
        ),
        (
            "SELECT Count(CopiedScore) AS CopiedScore INTO Archive FROM Items WHERE False",
            vec![Some(0)],
        ),
    ] {
        let prepared = prepare(&before, sql).unwrap();
        assert_eq!(
            prepared.rows(),
            values
                .iter()
                .map(|v| vec![v.map_or(Value::Null, Value::Int64)])
                .collect::<Vec<_>>()
        );
        let expected = populated::computed::int64_values(&before, "Archive", &values);
        same(
            prepared
                .finish(&mut budget(), &|| false)
                .unwrap()
                .after_bytes(),
            &expected,
        );
    }
}

#[cfg(feature = "journal")]
#[test]
fn declared_having_and_projection_inputs_are_data_not_inherited_native_fields() {
    let before = source();
    for (sql, value, selected) in [
        (
            "PARAMETERS [Min] LONG; SELECT CLng(Count(*)) AS CopiedScore INTO Archive FROM Items AS i GROUP BY i.CopiedScore HAVING Count(*)>=[Min] ORDER BY i.CopiedScore",
            Value::Int32(3),
            vec![Some(3)],
        ),
        (
            "PARAMETERS [Min] LONG; SELECT [Min] AS CopiedScore INTO Archive FROM Items AS i GROUP BY i.CopiedScore ORDER BY i.CopiedScore",
            Value::Int32(5),
            vec![Some(5), Some(5), Some(5)],
        ),
    ] {
        let parameters = [("Min", value)];
        let prepared = prepare_parameterized_native_make_table_with_native_comparison(
            image(&before),
            syntax(sql),
            &RefCell::new(budget()),
            &RefCell::new(budget()),
            &mut budget(),
            NativeMakeTableOptions {
                parameters: &parameters,
                execution: ExecutionOptions::default(),
                absent_like_mode: None,
                snapshot_limits: TableSnapshotLimits::default(),
            },
        )
        .unwrap();
        let expected = populated::numbers(&before, "Archive", &selected);
        same(
            prepared
                .finish(&mut budget(), &|| false)
                .unwrap()
                .after_bytes(),
            &expected,
        );
    }
}

#[cfg(feature = "journal")]
#[test]
fn direct_text_and_decimal_group_fields_keep_native_capacity_and_declared_scale() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let before = populated::text_decimals(
        &seed,
        "Items",
        &[
            (None, None),
            (Some("A"), Some(10_000)),
            (Some("a"), Some(10_000)),
        ],
    );
    let prepared = prepare_native_make_table_with_native_comparison(
        image(&before),
        syntax("SELECT i.Label,i.Amount,CLng(Count(*)) AS CopiedScore INTO Archive FROM Items AS i GROUP BY i.Label,i.Amount ORDER BY i.Label,i.Amount"),
        &RefCell::new(budget()),
        &RefCell::new(budget()),
        &mut budget(),
        ExecutionOptions::default(),
        TableSnapshotLimits::default(),
    )
    .unwrap();
    let expected = populated::computed::after(
        &before,
        "Archive",
        &[
            populated::computed::Row {
                label: None,
                amount: None,
                score: Some(1),
            },
            populated::computed::Row {
                label: Some("A"),
                amount: Some(10_000),
                score: Some(2),
            },
        ],
    );
    same(
        prepared
            .finish(&mut budget(), &|| false)
            .unwrap()
            .after_bytes(),
        &expected,
    );
}

#[test]
fn group_errors_late_having_and_top_omitted_errors_never_create_preparation() {
    let before = source();
    for sql in [
        "SELECT TOP 1 CLng(Count(*)) AS CopiedScore INTO Archive FROM Items GROUP BY CopiedScore HAVING 1/(Count(*)-2)>0 ORDER BY CopiedScore",
        "SELECT CLng(Sum(1/(CopiedScore-2))) AS CopiedScore INTO Archive FROM Items GROUP BY CopiedScore",
    ] {
        assert!(
            matches!(
                prepare(&before, sql),
                Err(NativeMakeTableError::Query(MakeTableError::Query(
                    QueryError::Evaluation(EvalError::DivisionByZero)
                )))
            ),
            "{sql}"
        );
    }
    for sql in [
        "SELECT CLng(Count(*)) AS CopiedScore INTO Archive FROM Items GROUP BY Count(*)",
        "SELECT CopiedScore INTO Archive FROM Items GROUP BY CopiedScore+1",
        "SELECT CLng(Count(*)) AS CopiedScore INTO Archive FROM Items GROUP BY (SELECT 1)",
        "SELECT CLng(Count(*)) AS CopiedScore INTO Archive FROM Items GROUP BY CopiedScore HAVING UnknownFunction(1)>0",
        "SELECT CStr(Count(*)) AS Label INTO Archive FROM Items GROUP BY CopiedScore",
    ] {
        assert!(prepare(&before, sql).is_err(), "{sql}");
    }
}
