use super::*;

fn expected(left: &[Option<i32>], right: &[Option<i32>], join: &str) -> Vec<Vec<Value>> {
    let mut output = Vec::new();
    let mut matched = vec![false; right.len()];
    for (left_index, key) in left.iter().enumerate() {
        let mut hit = false;
        for (right_index, other) in right.iter().enumerate() {
            if key.is_some() && key == other {
                hit = true;
                matched[right_index] = true;
                output.push(vec![
                    Value::Int32(left_index as i32),
                    Value::Int32(right_index as i32),
                    Value::Text(format!("L{left_index}")),
                    Value::Text(format!("R{right_index}")),
                ]);
            }
        }
        if !hit && join == "LEFT" {
            output.push(vec![
                Value::Int32(left_index as i32),
                Value::Null,
                Value::Text(format!("L{left_index}")),
                Value::Null,
            ]);
        }
    }
    if join == "RIGHT" {
        for (right_index, hit) in matched.into_iter().enumerate() {
            if !hit {
                output.push(vec![
                    Value::Null,
                    Value::Int32(right_index as i32),
                    Value::Null,
                    Value::Text(format!("R{right_index}")),
                ]);
            }
        }
    }
    output
}

#[test]
fn same_carrier_nullable_duplicates_preserve_every_join_and_operand_order() {
    let left = [None, Some(1), Some(0), Some(1), Some(0)];
    let right = [Some(1), None, Some(1)];
    for kind in [
        ScalarType::Boolean,
        ScalarType::Byte,
        ScalarType::Int16,
        ScalarType::Int32,
        ScalarType::Int64,
    ] {
        for declared_integral in [false, true] {
            for join in ["INNER", "LEFT", "RIGHT"] {
                for equality in ["L.K=R.K", "R.K=L.K"] {
                    let source = tables(kind, &left, &right);
                    let sql = format!(
                        "SELECT L.Id,R.Id,L.Payload,R.Payload
                         FROM L {join} JOIN R ON {equality}"
                    );
                    let rows = execute(
                        &sql,
                        &source,
                        ExecutionOptions {
                            declared_integral,
                            ..Default::default()
                        },
                    )
                    .unwrap();
                    assert_eq!(rows, expected(&left, &right, join));
                    assert_eq!(source.scans.get(), 2);
                }
            }
        }
    }
}

#[test]
fn empty_sides_keep_existing_outer_join_encounter_contract() {
    for (left, right) in [
        (vec![], vec![None, Some(1)]),
        (vec![None, Some(0)], vec![]),
        (vec![], vec![]),
    ] {
        for join in ["INNER", "LEFT", "RIGHT"] {
            let source = tables(ScalarType::Int32, &left, &right);
            let sql = format!(
                "SELECT L.Id,R.Id,L.Payload,R.Payload
                 FROM L {join} JOIN R ON L.K=R.K"
            );
            let rows = execute(&sql, &source, Default::default()).unwrap();
            assert_eq!(rows, expected(&left, &right, join));
            assert_eq!(source.scans.get(), 2);
        }
    }
}

#[test]
fn eleven_hundred_matching_rows_fit_without_a_million_discarded_rows() {
    let keys = (0..1100).map(Some).collect::<Vec<_>>();
    let source = tables(ScalarType::Int32, &keys, &keys);
    let rows = execute(
        "SELECT L.K FROM L INNER JOIN R ON L.K=R.K",
        &source,
        ExecutionOptions {
            limits: QueryLimits {
                rows: 20_000,
                steps: QueryLimits::MAX_STEPS,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        rows,
        (0..1100)
            .map(|key| vec![Value::Int32(key)])
            .collect::<Vec<_>>()
    );
    assert_eq!(source.scans.get(), 2);
}

#[test]
fn field_proof_uses_pruned_and_nested_available_slot_positions() {
    let source = tables(
        ScalarType::Int32,
        &[Some(9), Some(8), Some(7)],
        &[Some(1), Some(2), Some(3)],
    );
    // K precedes Id in both stored schemas but is absent from every projection
    // and predicate. Looking up K instead of the rewritten Id would lose matches.
    for declared_integral in [false, true] {
        let options = ExecutionOptions {
            declared_integral,
            ..Default::default()
        };
        let rows = execute(
            "SELECT L.Payload,R.Payload FROM L INNER JOIN R ON L.Id=R.Id",
            &source,
            options,
        )
        .unwrap();
        assert_eq!(
            rows,
            (0..3)
                .map(|index| {
                    vec![
                        Value::Text(format!("L{index}")),
                        Value::Text(format!("R{index}")),
                    ]
                })
                .collect::<Vec<_>>()
        );

        let derived = execute(
            "SELECT A.Id,B.Id FROM (SELECT Id,Payload FROM L) A
             INNER JOIN (SELECT Id,Payload FROM R) B ON A.Id=B.Id",
            &source,
            options,
        )
        .unwrap();
        assert_eq!(
            derived,
            (0..3)
                .map(|index| vec![Value::Int32(index), Value::Int32(index)])
                .collect::<Vec<_>>()
        );

        let nested = execute(
            "SELECT L.Id,R.Id,S.Id FROM (L INNER JOIN R ON L.Id=R.Id)
             INNER JOIN L AS S ON R.Id=S.Id",
            &source,
            options,
        )
        .unwrap();
        assert_eq!(
            nested,
            (0..3)
                .map(|index| vec![Value::Int32(index); 3])
                .collect::<Vec<_>>()
        );
    }
}
