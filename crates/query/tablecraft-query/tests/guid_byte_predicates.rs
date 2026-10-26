//! GUID-structure predicate coercion does not redefine stored values or strict keys.
use std::cell::Cell;
use tablecraft_core::values::{Guid, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    BindingDepth, ExecutionOptions, PreparationOptions, QueryError, prepare_with_options,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

const TEXT: &str = "00112233-4455-6677-8899-aabbccddeeff";

fn data() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![
                Column::new("ID".into(), ScalarType::Int32, false).unwrap(),
                Column::new("G".into(), ScalarType::Guid, true).unwrap(),
            ],
            vec![
                vec![
                    Value::Int32(1),
                    Value::Guid(Guid::from_bytes([
                        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb,
                        0xcc, 0xdd, 0xee, 0xff,
                    ])),
                ],
                vec![Value::Int32(2), Value::Guid(Guid::from_bytes([0xff; 16]))],
                vec![Value::Int32(3), Value::Null],
            ],
        )
        .unwrap(),
    ])
    .unwrap()
}

#[test]
fn equality_membership_subqueries_and_joins_share_guid_byte_coercion() {
    let source = data();
    let bytes = format!("GUIDFromString('{TEXT}')");
    for predicate in [
        format!("G={bytes}"),
        format!("{bytes}=G"),
        format!("G IN ({bytes})"),
        format!("G IN (SELECT {bytes})"),
        format!("G=ANY(SELECT {bytes})"),
    ] {
        let syntax = parse_query(
            &format!("SELECT ID FROM T WHERE {predicate}"),
            Default::default(),
        )
        .unwrap();
        let plan = prepare_with_options(
            syntax.query(),
            &source,
            PreparationOptions {
                binding_depth: BindingDepth::new(32).unwrap(),
                ..Default::default()
            },
        )
        .unwrap();
        for declared_integral in [false, true] {
            let result = plan
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        declared_integral,
                        ..Default::default()
                    },
                )
                .unwrap();
            assert_eq!(result.rows(), [vec![Value::Int32(1)]], "{predicate}");
        }
    }
    let sql = format!("SELECT A.ID FROM T A INNER JOIN (SELECT {bytes} AS B) Q ON A.G=Q.B");
    let syntax = parse_query(&sql, Default::default()).unwrap();
    let plan = prepare_with_options(
        syntax.query(),
        &source,
        PreparationOptions {
            binding_depth: BindingDepth::new(32).unwrap(),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        plan.execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        [vec![Value::Int32(1)]]
    );
}

#[test]
fn domain_having_and_prepared_parameters_reuse_predicates_without_changing_carriers() {
    let source = data();
    for sql in [
        format!("SELECT Count(*) FROM T HAVING First(G)=GUIDFromString('{TEXT}')"),
        format!("SELECT DCount('*','T','G=GUIDFromString(\"{TEXT}\")')"),
    ] {
        let syntax = parse_query(&sql, Default::default()).unwrap();
        let plan = prepare_with_options(
            syntax.query(),
            &source,
            PreparationOptions {
                binding_depth: BindingDepth::new(32).unwrap(),
                ..Default::default()
            },
        )
        .unwrap();
        let result = plan.execute(&source, &[], Default::default()).unwrap();
        let expected = if sql.contains("DCount") { 1 } else { 3 };
        assert_eq!(result.rows(), [vec![Value::Int64(expected)]]);
    }
    let syntax = parse_query(
        "PARAMETERS P TEXT; SELECT ID,G FROM T WHERE G=GUIDFromString(P)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare_with_options(syntax.query(), &source, Default::default()).unwrap();
    for (text, identity) in [
        (TEXT, 1),
        ("ffffffff-ffff-ffff-ffff-ffffffffffff", 2),
        (TEXT, 1),
    ] {
        let result = plan
            .execute(
                &source,
                &[("P", Value::Text(text.into()))],
                Default::default(),
            )
            .unwrap();
        assert_eq!(result.rows()[0][0], Value::Int32(identity));
        assert!(matches!(result.rows()[0][1], Value::Guid(_)));
    }
}

#[test]
fn strict_keys_and_eager_failures_keep_their_separate_boundaries() {
    let source = data();
    for sql in [
        format!(
            "SELECT Count(*) FROM (SELECT G AS K FROM T WHERE ID=1 UNION ALL SELECT GUIDFromString('{TEXT}')) Q GROUP BY K"
        ),
        "SELECT ID FROM T WHERE False AND G=GUIDFromString('bad')".into(),
    ] {
        let syntax = parse_query(&sql, Default::default()).unwrap();
        let plan = prepare_with_options(
            syntax.query(),
            &source,
            PreparationOptions {
                binding_depth: BindingDepth::new(32).unwrap(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            plan.execute(&source, &[], Default::default()).err(),
            Some(QueryError::Evaluation(EvalError::TypeMismatch))
        );
    }
}

#[test]
fn every_observed_guid_filter_checkpoint_can_cancel_without_reusing_partial_results() {
    let source = data();
    let sql = format!("SELECT ID FROM T WHERE G=GUIDFromString('{TEXT}')");
    let syntax = parse_query(&sql, Default::default()).unwrap();
    let plan = prepare_with_options(syntax.query(), &source, Default::default()).unwrap();
    let visits = Cell::new(0);
    let count = || {
        visits.set(visits.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&count),
            ..Default::default()
        },
    )
    .unwrap();
    let total = visits.get();
    for boundary in 1..=total {
        visits.set(0);
        let cancelled = || {
            visits.set(visits.get() + 1);
            visits.get() == boundary
        };
        assert!(matches!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    cancelled: Some(&cancelled),
                    ..Default::default()
                }
            ),
            Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }
    assert_eq!(
        plan.execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        [vec![Value::Int32(1)]]
    );
}

#[test]
fn progressive_guid_filters_match_buffered_execution() {
    let source = data();
    let sql = format!("SELECT ID FROM T WHERE G=GUIDFromString('{TEXT}')");
    let syntax = parse_query(&sql, Default::default()).unwrap();
    let plan = prepare_with_options(syntax.query(), &source, Default::default()).unwrap();
    let mut rows = Vec::new();
    let summary = plan
        .execute_progressive(&source, &[], Default::default(), &mut |row| {
            rows.push(row.to_vec());
            Ok::<(), ()>(())
        })
        .unwrap();
    assert_eq!(rows, [vec![Value::Int32(1)]]);
    assert_eq!(summary.scanned_rows, 3);
    assert_eq!(summary.emitted_rows, 1);
}
