use super::*;
use std::cmp::Ordering;
use tablecraft_expressions::comparison::{Collation, StringComparisonProfile};
use tablecraft_query::source::CollationResolver;

fn model(
    left: &[Option<i128>],
    right: &[Option<i128>],
    join: &str,
) -> Vec<(Option<usize>, Option<usize>)> {
    let mut output = Vec::new();
    let mut matched_right = vec![false; right.len()];
    for (left_index, left_key) in left.iter().enumerate() {
        let matching: Vec<_> = right
            .iter()
            .enumerate()
            .filter_map(|(right_index, right_key)| match (left_key, right_key) {
                (Some(left), Some(right)) if left == right => Some(right_index),
                _ => None,
            })
            .collect();
        if matching.is_empty() && join == "LEFT" {
            output.push((Some(left_index), None));
        }
        for right_index in matching {
            matched_right[right_index] = true;
            output.push((Some(left_index), Some(right_index)));
        }
    }
    if join == "RIGHT" {
        output.extend(
            matched_right
                .iter()
                .enumerate()
                .filter_map(|(index, matched)| (!matched).then_some((None, Some(index)))),
        );
    }
    output
}

#[test]
fn five_carriers_follow_independent_duplicate_outer_null_order_in_both_profiles_and_directions() {
    for (kind, low, high, middle, other) in [
        (ScalarType::Boolean, 0, 1, 0, 1),
        (ScalarType::Byte, 0, 255, 128, 1),
        (ScalarType::Int16, -32768, 32767, 0, -1),
        (ScalarType::Int32, -2147483648, 2147483647, 0, -1),
        (
            ScalarType::Int64,
            -9223372036854775808,
            9223372036854775807,
            9007199254740993,
            9007199254740992,
        ),
    ] {
        let left = [
            Some(high),
            None,
            Some(low),
            Some(high),
            Some(middle),
            Some(low),
        ];
        let right = [
            Some(low),
            Some(high),
            Some(low),
            None,
            Some(high),
            Some(other),
        ];
        for declared_integral in [false, true] {
            for join in ["INNER", "LEFT", "RIGHT"] {
                for condition in ["L.K=R.K", "R.K=L.K"] {
                    let source = Inputs::new(kind, &left, &right);
                    let expected: Vec<_> = model(&left, &right, join)
                        .into_iter()
                        .map(|(l, r)| {
                            let cell = |side: usize, position: Option<usize>, column: usize| {
                                position.map_or(Value::Null, |index| {
                                    source.tables[side].rows[index][column].clone()
                                })
                            };
                            vec![
                                cell(0, l, 2),
                                cell(1, r, 2),
                                cell(0, l, 1),
                                cell(1, r, 1),
                                cell(0, l, 3),
                                cell(1, r, 3),
                            ]
                        })
                        .collect();
                    let sql = format!(
                        "SELECT L.Id,R.Id,L.K,R.K,L.Payload,R.Payload FROM L {join} JOIN R ON {condition}"
                    );
                    let query = parse_query(&sql, Default::default()).unwrap();
                    let plan = prepare(query.query(), &source).unwrap();
                    assert_eq!(source.scans[0].get() + source.scans[1].get(), 0);
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
                    assert_eq!(
                        result.columns(),
                        ["Id", "Id", "K", "K", "Payload", "Payload"]
                    );
                    for (actual, (kind, nullable)) in result.output_types().iter().zip([
                        (ScalarType::Int32, join == "RIGHT"),
                        (ScalarType::Int32, join == "LEFT"),
                        (kind, true),
                        (kind, true),
                        (ScalarType::Text, join == "RIGHT"),
                        (ScalarType::Text, join == "LEFT"),
                    ]) {
                        assert!(
                            matches!(actual, OutputType::Scalar { data_type, nullable: actual_null }
                            if data_type.scalar_type() == kind && *actual_null == nullable)
                        );
                    }
                    assert_eq!(result.rows(), expected, "{kind:?}/{join}/{condition}");
                    assert_eq!((source.scans[0].get(), source.scans[1].get()), (1, 1));
                    assert!(
                        source
                            .projections
                            .borrow()
                            .iter()
                            .all(|(_, indices)| !indices.contains(&0))
                    );
                }
            }
        }
    }
}

#[test]
fn empty_sides_and_nested_pruned_slot_positions_keep_their_existing_encounter_order() {
    for (left, right) in [
        (vec![], vec![None, Some(1)]),
        (vec![None, Some(0)], vec![]),
        (vec![], vec![]),
    ] {
        for join in ["INNER", "LEFT", "RIGHT"] {
            let source = Inputs::new(ScalarType::Int32, &left, &right);
            let expected: Vec<_> = model(&left, &right, join)
                .into_iter()
                .map(|(l, r)| {
                    vec![
                        l.map_or(Value::Null, |value| Value::Int32(value as i32)),
                        r.map_or(Value::Null, |value| Value::Int32(value as i32)),
                    ]
                })
                .collect();
            assert_eq!(
                execute(
                    &format!("SELECT L.Id,R.Id FROM L {join} JOIN R ON R.K=L.K"),
                    &source,
                    Default::default()
                )
                .unwrap(),
                expected
            );
        }
    }
    let source = Inputs::new(
        ScalarType::Int32,
        &[Some(9), Some(8), Some(7)],
        &[Some(1), Some(2), Some(3)],
    );
    for sql in [
        "SELECT L.Id,R.Id FROM L INNER JOIN R ON L.Id=R.Id",
        "SELECT A.Id,B.Id FROM (SELECT Id,Payload FROM L) A INNER JOIN (SELECT Id,Payload FROM R) B ON B.Id=A.Id",
        "SELECT L.Id,R.Id FROM (L INNER JOIN R ON L.Id=R.Id) INNER JOIN L S ON R.Id=S.Id",
    ] {
        assert_eq!(
            execute(sql, &source, Default::default()).unwrap(),
            (0..3)
                .map(|value| vec![Value::Int32(value); 2])
                .collect::<Vec<_>>()
        );
    }
}

struct Fold;
impl Collation for Fold {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left
            .bytes()
            .map(|byte| byte.to_ascii_lowercase())
            .cmp(right.bytes().map(|byte| byte.to_ascii_lowercase())))
    }

    fn string_comparison_profile(&self) -> StringComparisonProfile {
        StringComparisonProfile::Utf16
    }

    fn compare_utf16(&self, left: &[u16], right: &[u16]) -> Result<Ordering, EvalError> {
        let fold = |unit: &u16| {
            if (65..=90).contains(unit) {
                unit + 32
            } else {
                *unit
            }
        };
        Ok(left.iter().map(fold).cmp(right.iter().map(fold)))
    }
}

struct Resolver(RefCell<Vec<u64>>);
impl CollationResolver for Resolver {
    fn resolve(&self, policy: TextCollationId) -> Result<&dyn Collation, QueryError> {
        self.0.borrow_mut().push(policy.host_key());
        assert_eq!(policy.host_key(), 11);
        Ok(&Fold)
    }
}

#[test]
fn joined_payloads_keep_raw_utf16_and_field_policy_through_derived_available_slots() {
    let mut source = Inputs::new(ScalarType::Int32, &[Some(1), Some(1)], &[Some(1)]);
    source.tables[0].rows[0][3] = Value::Text("A".into());
    source.tables[0].rows[1][3] = Value::Text("a".into());
    for declared_integral in [false, true] {
        let resolver = Resolver(RefCell::new(Vec::new()));
        let sql = "SELECT A.Id,LenB(A.Raw)
                   FROM (SELECT Id,K,ChrW(55296)&Payload AS Raw FROM L) A
                   INNER JOIN R ON R.K=A.K WHERE A.Raw=ChrW(55296)&'a'";
        let rows = execute(
            sql,
            &source,
            ExecutionOptions {
                declared_integral,
                collation_resolver: Some(&resolver),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            rows,
            [
                vec![Value::Int32(0), Value::Int32(4)],
                vec![Value::Int32(1), Value::Int32(4)]
            ]
        );
        let policies = resolver.0.borrow();
        assert!(!policies.is_empty());
        assert!(policies.iter().all(|key| *key == 11));
    }
}

#[test]
fn derived_same_int16_arithmetic_keys_keep_both_active_declaration_profiles() {
    let source = Inputs::new(ScalarType::Int16, &[Some(1), Some(2)], &[Some(2), Some(1)]);
    for declared_integral in [false, true] {
        let sql = "SELECT A.Id,B.Id FROM (SELECT Id,K+K AS Twice FROM L) A
                   INNER JOIN (SELECT Id,K+K AS Twice FROM R) B ON B.Twice=A.Twice";
        let rows = execute(
            sql,
            &source,
            ExecutionOptions {
                declared_integral,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            rows,
            [
                vec![Value::Int32(0), Value::Int32(1)],
                vec![Value::Int32(1), Value::Int32(0)]
            ]
        );
    }
}
