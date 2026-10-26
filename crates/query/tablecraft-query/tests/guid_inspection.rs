//! Stored GUID inspection uses a fixed Byte-array view without rewriting source cells.
use std::cell::Cell;
use tablecraft_core::values::{Guid, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    BindingDepth, ExecutionOptions, PreparationOptions, PreparedQuery, QueryError,
    prepare_with_options,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![
                Column::new("ID".into(), ScalarType::Int32, false).unwrap(),
                Column::new("G".into(), ScalarType::Guid, true).unwrap(),
            ],
            vec![
                vec![Value::Int32(1), Value::Guid(Guid::from_bytes([0x12; 16]))],
                vec![Value::Int32(2), Value::Null],
                vec![Value::Int32(3), Value::Guid(Guid::from_bytes([0xff; 16]))],
            ],
        )
        .unwrap(),
    ])
    .unwrap()
}

fn prepare(sql: &str, data: &MemorySource) -> PreparedQuery {
    let syntax = parse_query(sql, Default::default()).unwrap();
    prepare_with_options(
        syntax.query(),
        data,
        PreparationOptions {
            binding_depth: BindingDepth::new(32).unwrap(),
            ..Default::default()
        },
    )
    .unwrap()
}

#[test]
fn stored_guid_type_and_bounds_work_in_buffered_and_progressive_queries() {
    let data = source();
    let plan = prepare(
        "SELECT ID,TypeName(G),VarType(G),IsArray(G),LBound(G),UBound(G),G FROM T WHERE G IS NOT NULL",
        &data,
    );
    let expected = [(1, 0x12), (3, 0xff)].map(|(identity, byte)| {
        vec![
            Value::Int32(identity),
            Value::Text("Byte()".into()),
            Value::Int16(8209),
            Value::Boolean(true),
            Value::Int32(0),
            Value::Int32(15),
            Value::Guid(Guid::from_bytes([byte; 16])),
        ]
    });
    assert_eq!(
        plan.execute(&data, &[], Default::default()).unwrap().rows(),
        expected
    );
    let mut streamed = Vec::new();
    let summary = plan
        .execute_progressive(&data, &[], Default::default(), &mut |row| {
            streamed.push(row.to_vec());
            Ok::<(), ()>(())
        })
        .unwrap();
    assert_eq!(streamed, expected);
    assert_eq!(summary.scanned_rows, 3);
    assert_eq!(summary.emitted_rows, 2);
}

#[test]
fn parameters_selectors_subqueries_domains_and_groups_share_guid_views() {
    let data = source();
    let parameter = prepare(
        "PARAMETERS P GUID; SELECT TypeName(CVar(P)),VarType((SELECT P)),LBound(IIf(True,P,P)),UBound(P)",
        &data,
    );
    for bytes in [[0; 16], [0xff; 16], [0x12; 16]] {
        let result = parameter
            .execute(
                &data,
                &[("P", Value::Guid(Guid::from_bytes(bytes)))],
                Default::default(),
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            [vec![
                Value::Text("Byte()".into()),
                Value::Int16(8209),
                Value::Int32(0),
                Value::Int32(15),
            ]]
        );
    }
    let plan = prepare(
        "SELECT TypeName(First(G)),VarType(G),Count(*) FROM T WHERE G IS NOT NULL GROUP BY VarType(G),G",
        &data,
    );
    let result = plan.execute(&data, &[], Default::default()).unwrap();
    assert_eq!(
        result.rows(),
        vec![
            vec![
                Value::Text("Byte()".into()),
                Value::Int16(8209),
                Value::Int64(1),
            ];
            2
        ]
    );

    let domain = prepare("SELECT UBound(DFirst('G','T','ID=1'))", &data);
    assert_eq!(
        domain
            .execute(&data, &[], Default::default())
            .unwrap()
            .rows(),
        [vec![Value::Int32(15)]]
    );
}

#[test]
fn null_dimensions_and_eager_operational_failures_keep_their_existing_errors() {
    let data = source();
    for (sql, error) in [
        (
            "SELECT UBound(G) FROM T WHERE ID=2",
            EvalError::InvalidNullConversion,
        ),
        (
            "SELECT UBound(G,2) FROM T WHERE ID=1",
            EvalError::ArrayBounds,
        ),
        (
            "SELECT UBound(G,1/0) FROM T WHERE ID=2",
            EvalError::DivisionByZero,
        ),
        (
            "SELECT IIf(False,UBound(G,2),1) FROM T WHERE ID=1",
            EvalError::ArrayBounds,
        ),
    ] {
        let plan = prepare(sql, &data);
        assert_eq!(
            plan.execute(&data, &[], Default::default()).err(),
            Some(QueryError::Evaluation(error))
        );
    }
    let plan = prepare(
        "SELECT TypeName(G),VarType(G),IsArray(G) FROM T WHERE ID=2",
        &data,
    );
    assert_eq!(
        plan.execute(&data, &[], Default::default()).unwrap().rows(),
        [vec![
            Value::Text("Null".into()),
            Value::Int16(1),
            Value::Boolean(false),
        ]]
    );
}

#[test]
fn every_observed_inspection_checkpoint_can_cancel_without_retained_views() {
    let data = source();
    let plan = prepare(
        "SELECT TypeName(G),VarType(G),LBound(G),UBound(G) FROM T WHERE G IS NOT NULL",
        &data,
    );
    let visits = Cell::new(0);
    let count = || {
        visits.set(visits.get() + 1);
        false
    };
    plan.execute(
        &data,
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
                &data,
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
        plan.execute(&data, &[], Default::default())
            .unwrap()
            .rows()
            .len(),
        2
    );
}
