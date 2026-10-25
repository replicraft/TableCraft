use super::*;
use tablecraft_query::{OutputType, PreparationOptions, prepare_with_options};

pub(super) const NUMERIC_SQL: &str = "SELECT CLng(Id Mod 17) AS K,Count(*) AS N,
    Count(Score),Sum(Score),Avg(Score),Min(Score),Max(Score)
    FROM T GROUP BY Id Mod 17";

struct Totals {
    key: i32,
    count: i64,
    nonnull: i64,
    sum: i64,
    minimum: Option<i32>,
    maximum: Option<i32>,
}

pub(super) fn model(first: i32, count: usize, divisor: i32) -> Vec<Vec<Value>> {
    let mut groups: Vec<Totals> = Vec::new();
    for ordinal in 0..count {
        let id = first + i32::try_from(ordinal).unwrap();
        let group_key = remainder(id, divisor);
        let position = match groups.iter().position(|group| group.key == group_key) {
            Some(position) => position,
            None => {
                groups.push(Totals {
                    key: group_key,
                    count: 0,
                    nonnull: 0,
                    sum: 0,
                    minimum: None,
                    maximum: None,
                });
                groups.len() - 1
            }
        };
        let group = &mut groups[position];
        group.count += 1;
        if remainder(id, 11) != 0 {
            let score = remainder(id, 31) - 15;
            group.nonnull += 1;
            group.sum += i64::from(score);
            group.minimum = Some(group.minimum.map_or(score, |value| value.min(score)));
            group.maximum = Some(group.maximum.map_or(score, |value| value.max(score)));
        }
    }
    groups
        .into_iter()
        .map(|group| {
            let (sum, average) = if group.nonnull == 0 {
                (Value::Null, Value::Null)
            } else {
                (
                    Value::Int64(group.sum),
                    Value::Double(Double::from_float(group.sum as f64 / group.nonnull as f64)),
                )
            };
            vec![
                Value::Int32(group.key),
                Value::Int64(group.count),
                Value::Int64(group.nonnull),
                sum,
                average,
                group.minimum.map_or(Value::Null, Value::Int32),
                group.maximum.map_or(Value::Null, Value::Int32),
            ]
        })
        .collect()
}

#[test]
fn signed_modulo_groups_have_independent_mixed_aggregate_answers_and_small_buffered_parity() {
    for (first, count) in [(-25, 53), (11, 1), (1, 100)] {
        let mut source = Inputs::generated(first, count);
        let expected = model(first, count, 17);
        for declared_integral in [false, true] {
            let options = ExecutionOptions {
                declared_integral,
                ..Default::default()
            };
            assert_eq!(stream(NUMERIC_SQL, &source, options).unwrap(), expected);
            source.allow_buffered = true;
            let query = parse_query(NUMERIC_SQL, Default::default()).unwrap();
            let result = prepare(query.query(), &source)
                .unwrap()
                .execute(&source, &[], options)
                .unwrap();
            assert_eq!(result.rows(), expected);
            source.allow_buffered = false;
        }
        assert!(
            source
                .projections
                .borrow()
                .iter()
                .all(|indices| indices == &[1, 3])
        );
    }
}

#[test]
fn direct_natural_modulo_keys_retain_literal_capture_carriers_and_paired_metadata() {
    let source = Inputs::generated(1, 4);
    let sql = "SELECT Id Mod 17 AS K,Count(*) AS N FROM T GROUP BY Id Mod 17";
    for prepare_declared in [false, true] {
        let query = parse_query(sql, Default::default()).unwrap();
        let plan = prepare_with_options(
            query.query(),
            &source,
            PreparationOptions {
                declared_arithmetic: prepare_declared,
                ..Default::default()
            },
        )
        .unwrap();
        let kind = if prepare_declared {
            ScalarType::Int32
        } else {
            ScalarType::Int64
        };
        assert_eq!(
            plan.output_types().unwrap(),
            [
                OutputType::Scalar {
                    data_type: TypeDescriptor::new(kind),
                    nullable: false
                },
                OutputType::Scalar {
                    data_type: TypeDescriptor::new(ScalarType::Int64),
                    nullable: false
                },
            ]
        );
        for execute_declared in [false, true] {
            let mut rows = Vec::new();
            plan.execute_progressive(
                &source,
                &[],
                ExecutionOptions {
                    declared_integral: execute_declared,
                    ..Default::default()
                },
                &mut |row| {
                    rows.push(row.to_vec());
                    Ok::<_, ()>(())
                },
            )
            .unwrap();
            let expected: Vec<_> = (1..=4)
                .map(|value| vec![key(kind, value), Value::Int64(1)])
                .collect();
            assert_eq!(rows, expected);
        }
    }
}

#[test]
fn every_exact_key_family_and_nullable_tuple_preserves_first_encounter_identity() {
    for kind in [
        ScalarType::Boolean,
        ScalarType::Byte,
        ScalarType::Int16,
        ScalarType::Int32,
        ScalarType::Int64,
    ] {
        let mut source = Inputs::generated(1, 20);
        source.group_kind = kind;
        source.columns[2] = Column::new("G".into(), kind, true).unwrap();
        let mut groups: Vec<(Option<i128>, i64)> = Vec::new();
        for id in 1..=20 {
            let group = if id % 7 == 0 {
                None
            } else {
                Some(i128::from(id % 2))
            };
            if let Some((_, count)) = groups.iter_mut().find(|(key, _)| *key == group) {
                *count += 1;
            } else {
                groups.push((group, 1));
            }
        }
        let expected: Vec<_> = groups
            .into_iter()
            .map(|(group, count)| {
                vec![
                    group.map_or(Value::Null, |value| key(kind, value)),
                    Value::Int64(count),
                ]
            })
            .collect();
        assert_eq!(
            stream(
                "SELECT G,Count(*) FROM T GROUP BY G",
                &source,
                Default::default()
            )
            .unwrap(),
            expected
        );
    }

    let source = Inputs::generated(-13, 29);
    let mut groups: Vec<((Option<i16>, i32), i64)> = Vec::new();
    for id in -13..=15 {
        let first = (remainder(id, 7) != 0).then_some(remainder(id, 2) as i16);
        let tuple = (first, remainder(id, 3));
        if let Some((_, count)) = groups.iter_mut().find(|(key, _)| *key == tuple) {
            *count += 1;
        } else {
            groups.push((tuple, 1));
        }
    }
    let expected: Vec<_> = groups
        .into_iter()
        .map(|((first, second), count)| {
            vec![
                first.map_or(Value::Null, Value::Int16),
                Value::Int32(second),
                Value::Int64(count),
            ]
        })
        .collect();
    assert_eq!(
        stream(
            "SELECT G,CLng(Id Mod 3),Count(*) FROM T GROUP BY G,Id Mod 3",
            &source,
            Default::default()
        )
        .unwrap(),
        expected
    );
}

#[test]
fn prior_aliases_read_retained_group_keys_and_empty_groups_never_invent_a_row() {
    for declared_integral in [false, true] {
        let source = Inputs::generated(1, 6);
        let rows = stream(
            "SELECT CLng(Id Mod 3) AS K,Count(*) AS N,CLng(K+1),CLng(N) FROM T GROUP BY Id Mod 3",
            &source,
            ExecutionOptions {
                declared_integral,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            rows,
            [1, 2, 0]
                .into_iter()
                .map(|key| vec![
                    Value::Int32(key),
                    Value::Int64(2),
                    Value::Int32(key + 1),
                    Value::Int32(2)
                ])
                .collect::<Vec<_>>()
        );
    }
    for source in [Inputs::generated(1, 0), Inputs::generated(1, 3)] {
        assert_eq!(
            stream(
                "SELECT CLng(1/0),Count(*) FROM T WHERE False GROUP BY CLng(1/0)",
                &source,
                Default::default()
            )
            .unwrap(),
            Vec::<Vec<Value>>::new()
        );
    }
    let source = Inputs::generated(1, 3);
    assert_eq!(
        stream(
            "SELECT NULL,Count(*) FROM T GROUP BY NULL",
            &source,
            Default::default()
        )
        .unwrap(),
        [vec![Value::Null, Value::Int64(3)]]
    );
}
