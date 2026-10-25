use super::*;
use std::collections::BTreeMap;

fn generated(count: i32) -> (Inputs, Vec<Vec<Value>>) {
    let left: Vec<_> = (0..count).rev().map(|key| Some(i128::from(key))).collect();
    let right: Vec<_> = (0..count)
        .map(|index| Some(i128::from((index * 37) % count)))
        .collect();
    let positions: BTreeMap<_, _> = right
        .iter()
        .enumerate()
        .map(|(index, key)| (key.unwrap(), index))
        .collect();
    let expected = left
        .iter()
        .enumerate()
        .map(|(index, key)| {
            let right_index = positions[&key.unwrap()];
            vec![Value::Int32(index as i32), Value::Int32(right_index as i32)]
        })
        .collect();
    (Inputs::new(ScalarType::Int32, &left, &right), expected)
}

#[test]
fn eleven_hundred_unique_rows_fit_default_steps_with_exact_encounter_order() {
    let (source, expected) = generated(1100);
    for condition in ["L.K=R.K", "R.K=L.K"] {
        let sql = format!("SELECT L.Id,R.Id FROM L INNER JOIN R ON {condition}");
        let rows = execute(&sql, &source, Default::default()).unwrap();
        assert_eq!(rows, expected);
    }
}

#[test]
fn twelve_thousand_unique_rows_fit_explicit_finite_work_and_storage_budgets() {
    let (source, expected) = generated(12000);
    for declared_integral in [false, true] {
        let rows = execute(
            "SELECT L.Id,R.Id FROM L INNER JOIN R ON R.K=L.K",
            &source,
            ExecutionOptions {
                declared_integral,
                limits: QueryLimits {
                    bytes: 128 * 1024 * 1024,
                    rows: 200_000,
                    steps: 5_000_000,
                },
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(rows, expected);
    }
}
