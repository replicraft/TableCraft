#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod base;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/decimal_indexes/fixture.rs"]
mod decimal_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/composite_indexes/fixture.rs"]
mod numeric_fixture;
use tablecraft_testkit::access as support;

use std::io::Cursor;
use tablecraft_access::NativeSource;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, QueryError, QueryLimits, Source, prepare};
use tablecraft_sql::parse_query;

const PAGE_BYTES: usize = 4096;
const DATA_PAGE: usize = 12;
const ROW_DIRECTORY_OFFSET: usize = 14;
const DECIMAL_VALUE_OFFSET: usize = 6;
const MAGIC_OFFSET: usize = 4;
const FORMAT_OFFSET: usize = 20;
const INPUT_ROWS: usize = 64;
const QUERY_ROWS: u64 = 1024;

fn source(bytes: Vec<u8>) -> NativeSource<'static, Cursor<Vec<u8>>> {
    NativeSource::new(
        Cursor::new(bytes),
        CatalogScope::new(1),
        ResourceBudget::new(64 * 1024 * 1024, 100_000, 1_000_000),
        &|| false,
    )
    .unwrap()
}

fn database(profile: u8, rows: &[(Option<i32>, i32)]) -> Vec<u8> {
    let values = rows
        .iter()
        .map(|(key, ordinal)| vec![key.map(i64::from), Some(i64::from(*ordinal))])
        .collect::<Vec<_>>();
    let mut bytes = numeric_fixture::database(profile, &[4, 4], &[(0, false)], &values, 0);
    if profile == 1 {
        bytes[MAGIC_OFFSET..FORMAT_OFFSET].copy_from_slice(b"Standard Jet DB\0");
    }
    bytes
}

fn execute(
    sql: &str,
    source: &dyn Source,
    declared: bool,
    rows: u64,
) -> Result<Vec<Vec<Value>>, QueryError> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(syntax.query(), source)?
        .execute(
            source,
            &[],
            ExecutionOptions {
                declared_integral: declared,
                limits: QueryLimits {
                    rows,
                    steps: QueryLimits::MAX_STEPS,
                    ..Default::default()
                },
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn native_bare_self_join_preserves_physical_encounter_order_under_a_small_row_budget() {
    let input = (0..INPUT_ROWS)
        .map(|index| (Some(((index * 17) % INPUT_ROWS + 1) as i32), index as i32))
        .collect::<Vec<_>>();
    let expected = input
        .iter()
        .map(|(key, ordinal)| {
            vec![
                Value::Int32(key.unwrap()),
                Value::Int32(*ordinal),
                Value::Int32(*ordinal),
            ]
        })
        .collect::<Vec<_>>();
    for profile in [1, 2, 5, 6] {
        for declared in [false, true] {
            let native = source(database(profile, &input));
            for equality in ["L.Id=R.Id", "R.Id=L.Id"] {
                let sql = format!(
                    "SELECT L.Id,L.Score,R.Score FROM Items AS L INNER JOIN Items AS R ON {equality}",
                );
                assert_eq!(
                    execute(&sql, &native, declared, QUERY_ROWS).unwrap(),
                    expected
                );
            }

            // A compound predicate deliberately remains on the original eager path.
            // Its 4,096 candidate copies cannot fit this 1,024-row allowance.
            let compound = concat!(
                "SELECT L.Id,L.Score,R.Score FROM Items AS L INNER JOIN Items AS R ",
                "ON L.Id=R.Id AND True",
            );
            assert!(matches!(
                execute(compound, &native, declared, QUERY_ROWS),
                Err(QueryError::Limit(_)),
            ));
            assert_eq!(
                execute(compound, &native, declared, 20_000).unwrap(),
                expected
            );
        }
    }
}

fn expected_outer(
    left: &[(Option<i32>, i32)],
    right: &[(Option<i32>, i32)],
    kind: &str,
) -> Vec<Vec<Value>> {
    let mut rows = Vec::new();
    let mut matched = vec![false; right.len()];
    for (key, ordinal) in left {
        let mut hit = false;
        for (position, (other, other_ordinal)) in right.iter().enumerate() {
            if let (Some(key), Some(other)) = (key, other)
                && key == other
            {
                hit = true;
                matched[position] = true;
                rows.push(vec![
                    Value::Int32(*key),
                    Value::Int32(*other),
                    Value::Int32(*ordinal),
                    Value::Int32(*other_ordinal),
                ]);
            }
        }
        if !hit && kind == "LEFT" {
            rows.push(vec![
                key.map_or(Value::Null, Value::Int32),
                Value::Null,
                Value::Int32(*ordinal),
                Value::Null,
            ]);
        }
    }
    if kind == "RIGHT" {
        for ((key, ordinal), hit) in right.iter().zip(matched) {
            if !hit {
                rows.push(vec![
                    Value::Null,
                    key.map_or(Value::Null, Value::Int32),
                    Value::Null,
                    Value::Int32(*ordinal),
                ]);
            }
        }
    }
    rows
}

#[test]
fn native_nullable_duplicates_keep_outer_extension_and_unmatched_encounter_order() {
    let input = [
        (Some(3), 1),
        (None, 2),
        (Some(1), 3),
        (Some(3), 4),
        (Some(9), 5),
    ];
    let left = input
        .iter()
        .copied()
        .filter(|(_, score)| *score != 5)
        .collect::<Vec<_>>();
    let right = input
        .iter()
        .copied()
        .filter(|(_, score)| *score != 3)
        .collect::<Vec<_>>();
    for profile in [1, 2, 5, 6] {
        for declared in [false, true] {
            let native = source(database(profile, &input));
            for kind in ["INNER", "LEFT", "RIGHT"] {
                for equality in ["L.Id=R.Id", "R.Id=L.Id"] {
                    let sql = format!(
                        "SELECT L.Id,R.Id,L.Score,R.Score \
                         FROM (SELECT Id,Score FROM Items WHERE Score<>5) AS L \
                         {kind} JOIN (SELECT Id,Score FROM Items WHERE Score<>3) AS R \
                         ON {equality}",
                    );
                    assert_eq!(
                        execute(&sql, &native, declared, 1000).unwrap(),
                        expected_outer(&left, &right, kind)
                    );
                }
            }
        }
    }
}

#[test]
fn selected_invalid_native_payloads_are_not_hidden_by_empty_or_rejected_join_pairs() {
    for profile in [1, 2, 5, 6] {
        let mut bytes = decimal_fixture::database_for(
            profile,
            false,
            28,
            4,
            &[Some(10000), None, Some(30000)],
            0,
        );
        let directory = DATA_PAGE * PAGE_BYTES + ROW_DIRECTORY_OFFSET + 2 * size_of::<u16>();
        let last = usize::from(u16::from_le_bytes(
            bytes[directory..directory + size_of::<u16>()]
                .try_into()
                .unwrap(),
        ));
        bytes[DATA_PAGE * PAGE_BYTES + last + DECIMAL_VALUE_OFFSET] = 0x7f;
        let native = source(bytes);
        let pruned = "SELECT L.Id FROM Items AS L INNER JOIN Items AS R ON L.Id=R.Id";
        assert_eq!(
            execute(pruned, &native, false, 1000).unwrap(),
            [
                vec![Value::Int32(1)],
                vec![Value::Int32(2)],
                vec![Value::Int32(3)]
            ]
        );
        for sql in [
            "SELECT R.Score FROM Items AS L INNER JOIN Items AS R ON L.Id=R.Id",
            "SELECT R.Score FROM (SELECT Id FROM Items WHERE Id<0) AS L INNER JOIN Items AS R ON L.Id=R.Id",
        ] {
            assert_eq!(
                execute(sql, &native, false, 1000),
                Err(QueryError::Unsupported)
            );
        }
        assert_eq!(execute(pruned, &native, false, 1000).unwrap().len(), 3);
    }
}

#[path = "borrowed_join_equality/large.rs"]
mod large;
