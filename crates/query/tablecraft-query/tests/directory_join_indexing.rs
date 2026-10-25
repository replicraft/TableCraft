//! Whole proven equality uses bounded candidate work, with original SQL results.
use tablecraft_core::values::Value;
use tablecraft_query::{
    ExecutionOptions, QueryLimits, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

fn source(count: i32) -> MemorySource {
    let rows = (0..count)
        .rev()
        .map(|identity| vec![Value::Int32(identity)])
        .collect();
    let table = MemoryTable::new(
        "Items".into(),
        vec![Column::new("Id".into(), ScalarType::Int32, false).unwrap()],
        rows,
    )
    .unwrap();
    MemorySource::new(vec![table]).unwrap()
}

#[test]
fn ordinary_equality_join_fits_default_work_and_larger_explicit_allowances() {
    let syntax = parse_query(
        "SELECT L.Id,R.Id FROM Items L INNER JOIN Items R ON L.Id=R.Id",
        Default::default(),
    )
    .unwrap();
    for count in [1100, 12000] {
        let source = source(count);
        let prepared = prepare(syntax.query(), &source).unwrap();
        for declared_integral in [false, true] {
            let limits = if count == 1100 {
                QueryLimits::default()
            } else {
                QueryLimits {
                    bytes: 64 * 1024 * 1024,
                    rows: 200_000,
                    steps: 10_000_000,
                }
            };
            let result = prepared
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        declared_integral,
                        limits,
                        ..Default::default()
                    },
                )
                .unwrap();
            let expected = (0..count)
                .rev()
                .map(|identity| vec![Value::Int32(identity), Value::Int32(identity)])
                .collect::<Vec<_>>();
            assert_eq!(result.rows(), expected);
        }
    }
}

#[test]
fn directory_proof_uses_active_declared_descriptors_through_derived_reads() {
    let source = source(12000);
    let syntax = parse_query(
        "SELECT L.K,R.K FROM (SELECT CInt(Id)+CInt(0) AS K FROM Items) L
         INNER JOIN (SELECT CInt(Id)+CInt(0) AS K FROM Items) R ON L.K=R.K",
        Default::default(),
    )
    .unwrap();
    let prepared = prepare(syntax.query(), &source).unwrap();
    let result = prepared
        .execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                limits: QueryLimits {
                    bytes: 64 * 1024 * 1024,
                    rows: 200_000,
                    steps: 10_000_000,
                },
                ..Default::default()
            },
        )
        .unwrap();
    let expected = (0_i16..12000)
        .rev()
        .map(|identity| vec![Value::Int16(identity), Value::Int16(identity)])
        .collect::<Vec<_>>();
    assert_eq!(result.rows(), expected);
}
