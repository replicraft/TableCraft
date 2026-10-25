use super::*;

#[test]
fn union_distinct_prefixes_and_following_all_preserve_first_decimal_scale() {
    let keys = vec![decimal(100, 2), decimal(1, 0), decimal(2, 0), Value::Null];
    let rows: Vec<_> = keys.into_iter().map(|key| vec![key]).collect();
    let source = source(&[("K", ScalarType::Decimal)], rows.clone());
    let mut expected: Vec<_> = groups(&rows).into_iter().map(|(key, _)| key).collect();
    expected.extend(rows);
    for declared in [false, true] {
        let actual = buffered(
            "SELECT K FROM T UNION SELECT K FROM T UNION ALL SELECT K FROM T",
            &source,
            declared,
            declared,
        );
        assert_eq!(actual, expected);
    }
}

#[test]
fn progressive_fixed_keys_and_having_keep_actual_representatives_and_all_five_kernels() {
    for (kind, keys) in [
        (
            ScalarType::Decimal,
            vec![
                decimal(100, 2),
                decimal(1, 0),
                decimal(-200, 2),
                decimal(-2, 0),
                Value::Null,
                Value::Null,
            ],
        ),
        (
            ScalarType::Currency,
            vec![
                currency(i64::MIN),
                currency(i64::MIN),
                currency(1),
                currency(1),
                Value::Null,
                Value::Null,
            ],
        ),
    ] {
        let rows: Vec<_> = keys
            .into_iter()
            .enumerate()
            .map(|(index, key)| vec![key, Value::Int32(if index % 2 == 0 { 1 } else { 3 })])
            .collect();
        let key_rows: Vec<_> = rows.iter().map(|row| vec![row[0].clone()]).collect();
        let expected: Vec<_> = groups(&key_rows)
            .into_iter()
            .map(|(mut key, count)| {
                assert_eq!(count, 2);
                key.extend([
                    Value::Int64(2),
                    Value::Int64(4),
                    Value::Double(Double::from_float(2.0)),
                    Value::Int32(1),
                    Value::Int32(3),
                ]);
                key
            })
            .collect();
        let source = source(&[("K", kind), ("V", ScalarType::Int32)], rows);
        for prepare_declared in [false, true] {
            for declared_integral in [false, true] {
                let sql = "SELECT K,Count(*),Sum(V),Avg(V),Min(V),Max(V) FROM T GROUP BY K HAVING Count(*)>1";
                let syntax = parse_query(sql, Default::default()).unwrap();
                let plan = prepare_with_options(
                    syntax.query(),
                    &source,
                    PreparationOptions {
                        declared_arithmetic: prepare_declared,
                        ..Default::default()
                    },
                )
                .unwrap();
                let mut actual = Vec::new();
                let summary = plan
                    .execute_progressive(
                        &source,
                        &[],
                        ExecutionOptions {
                            declared_integral,
                            ..Default::default()
                        },
                        &mut |row| {
                            actual.push(row.to_vec());
                            Ok::<_, ()>(())
                        },
                    )
                    .unwrap();
                assert_eq!(actual, expected);
                assert_eq!(summary.scanned_rows, 6);
                assert_eq!(summary.emitted_rows, 3);
            }
        }
    }
}
