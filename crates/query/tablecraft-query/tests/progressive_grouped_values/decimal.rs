use super::*;

fn canonical(value: &Value) -> (i128, u8) {
    let Value::Decimal(value) = value else {
        panic!("Decimal aggregate lost carrier")
    };
    let mut coefficient = value.coefficient();
    let mut scale = value.scale();
    while scale != 0 && coefficient % 10 == 0 {
        coefficient /= 10;
        scale -= 1;
    }
    (coefficient, scale)
}

#[test]
fn interleaved_decimal_groups_keep_independent_exact_totals_and_half_even_means() {
    let maximum = 79228162514264337593543950335_i128;
    let amounts = [
        (2, maximum, 0),
        (0, 0, 28),
        (1, 1, 28),
        (2, maximum, 0),
        (0, 1, 28),
        (1, 2, 28),
        (2, -maximum, 0),
        (3, maximum, 0),
        (3, -maximum, 0),
        (3, 1, 28),
    ];
    let mut source = Inputs::generated(1, 0);
    source.columns[4] = Column::new("Amount".into(), ScalarType::Decimal, true).unwrap();
    source.population = Population::Stored(
        amounts
            .iter()
            .enumerate()
            .map(|(index, (group, coefficient, scale))| {
                vec![
                    Value::Text("pruned".into()),
                    Value::Int32(index as i32),
                    Value::Int16(*group),
                    Value::Null,
                    Value::Decimal(Decimal::new(*coefficient, *scale).unwrap()),
                ]
            })
            .chain(std::iter::once(vec![
                Value::Null,
                Value::Int32(10),
                Value::Null,
                Value::Null,
                Value::Null,
            ]))
            .collect(),
    );
    let rows = stream(
        "SELECT G,Count(Amount),Sum(Amount),Avg(Amount) FROM T GROUP BY G",
        &source,
        Default::default(),
    )
    .unwrap();
    for (row, group, count, sum, mean) in [
        (&rows[0], 2, 3, (maximum, 0), (maximum / 3, 0)),
        (&rows[1], 0, 2, (1, 28), (0, 0)),
        (&rows[2], 1, 2, (3, 28), (2, 28)),
        (&rows[3], 3, 3, (1, 28), (0, 0)),
    ] {
        assert_eq!(&row[..2], [Value::Int16(group), Value::Int64(count)]);
        assert_eq!(canonical(&row[2]), sum);
        assert_eq!(canonical(&row[3]), mean);
    }
    assert_eq!(
        rows[4],
        [Value::Null, Value::Int64(0), Value::Null, Value::Null]
    );
}
