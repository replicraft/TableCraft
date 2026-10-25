use super::*;
use std::ops::Range;
use tablecraft_core::values::Currency;
use tablecraft_query::ExecutionOptions;

const COEFFICIENTS: [Option<i128>; 6] =
    [Some(10000), None, Some(-10000), Some(10000), Some(0), None];
const CURRENCY_TYPE: u8 = 5;
const INT32_TYPE: u8 = 4;
const FIXED_SCALE: u8 = 4;
const ENGINE_SIGNATURE: Range<usize> = 4..20;

fn image(profile: u8, decimal: bool) -> Vec<u8> {
    if decimal {
        decimal_database(profile, &COEFFICIENTS, FIXED_SCALE)
    } else {
        let rows = COEFFICIENTS
            .iter()
            .enumerate()
            .map(|(index, value)| vec![Some(index as i64 + 1), value.map(|value| value as i64)])
            .collect::<Vec<_>>();
        let mut bytes = numeric_fixture::database(
            profile,
            &[INT32_TYPE, CURRENCY_TYPE],
            &[(1, false)],
            &rows,
            0,
        );
        if profile == 1 {
            bytes[ENGINE_SIGNATURE].copy_from_slice(b"Standard Jet DB\0");
        }
        bytes
    }
}

fn key(coefficient: Option<i128>, decimal: bool) -> Value {
    match coefficient {
        None => Value::Null,
        Some(coefficient) if decimal => {
            Value::Decimal(Decimal::new(coefficient, FIXED_SCALE).unwrap())
        }
        Some(coefficient) => Value::Currency(Currency::from_scaled(coefficient as i64)),
    }
}

#[test]
fn fixed_native_group_keys_and_having_keep_exact_carriers_without_complete_snapshots() {
    for profile in [1, 2, 5, 6] {
        for decimal in [false, true] {
            let bytes = image(profile, decimal);
            let source = native(bytes.clone(), 1, 1);
            let buffered = native(bytes, 100, 1);
            let expected = [
                vec![key(Some(10000), decimal), Value::Int64(2), Value::Int64(5)],
                vec![Value::Null, Value::Int64(2), Value::Int64(8)],
                vec![key(Some(-10000), decimal), Value::Int64(1), Value::Int64(3)],
                vec![key(Some(0), decimal), Value::Int64(1), Value::Int64(5)],
            ];
            for having in [false, true] {
                let sql = if having {
                    "SELECT Score,Count(*) AS N,Sum(Id) AS S FROM Items GROUP BY Score HAVING Count(*)>1"
                } else {
                    "SELECT Score,Count(*) AS N,Sum(Id) AS S FROM Items GROUP BY Score"
                };
                let query = plan(sql, &source);
                let expected = if having {
                    &expected[..2]
                } else {
                    &expected[..]
                };
                for declared_integral in [false, true] {
                    let options = ExecutionOptions {
                        declared_integral,
                        ..Default::default()
                    };
                    let mut rows = Vec::new();
                    let summary = query
                        .execute_progressive(&source, &[], options, &mut |row| {
                            rows.push(row.to_vec());
                            Ok::<_, ()>(())
                        })
                        .unwrap();
                    assert_eq!(rows, expected);
                    assert_eq!(summary.scanned_rows, 6);
                    assert_eq!(summary.emitted_rows, expected.len() as u64);
                    assert_eq!(
                        plan(sql, &buffered)
                            .execute(&buffered, &[], options)
                            .unwrap()
                            .rows(),
                        expected,
                    );
                }
            }
            assert_eq!(
                source.scan("Items", &mut |_| Ok(())),
                Err(QueryError::StructureLimit)
            );
        }
    }
}

#[test]
fn fixed_group_result_publication_still_waits_for_checked_native_eof() {
    for profile in [1, 2, 5, 6] {
        for decimal in [false, true] {
            let mut bytes = image(profile, decimal);
            support::put32(&mut bytes, TABLE_PAGE * PAGE_BYTES + ROW_COUNT_OFFSET, 7);
            let source = native(bytes, 1, 1);
            for having in ["", " HAVING Count(*)>0"] {
                let sql = format!("SELECT Score,Count(*) FROM Items GROUP BY Score{having}");
                let query = plan(&sql, &source);
                let mut rows = 0;
                assert!(
                    query
                        .execute_progressive(&source, &[], Default::default(), &mut |_| {
                            rows += 1;
                            Ok::<_, ()>(())
                        })
                        .is_err()
                );
                assert_eq!(rows, 0);
            }
        }
    }
}
