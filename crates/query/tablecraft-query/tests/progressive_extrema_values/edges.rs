use super::*;

fn buffered(source: &Data, sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), source).unwrap();
    Ok(plan.execute(source, &[], options(false))?.rows().to_vec())
}

#[test]
fn extended_dates_and_guid_binary_singletons_preserve_current_admission_boundaries() {
    let earlier =
        Value::DateTimeExtended(ExtendedDateTime::new(2026, 10, 25, 12, 0, 0, 1).unwrap());
    let later = Value::DateTimeExtended(ExtendedDateTime::new(2026, 10, 25, 12, 0, 0, 2).unwrap());
    let source = Data::new(
        ScalarType::DateTimeExtended,
        vec![later.clone(), earlier.clone()],
    );
    let sql = "SELECT Min(N),Max(N) FROM T";
    let expected = [vec![earlier, later]];
    assert_eq!(buffered(&source, sql).unwrap(), expected);
    assert_eq!(stream(&source, sql, options(false)).unwrap(), expected);

    for (kind, value, different) in [
        (
            ScalarType::Guid,
            Value::Guid(Guid::from_bytes([1; 16])),
            Value::Guid(Guid::from_bytes([2; 16])),
        ),
        (
            ScalarType::Binary,
            Value::Binary(vec![1, 2]),
            Value::Binary(vec![2, 3]),
        ),
    ] {
        let singleton = Data::new(kind, vec![value.clone()]);
        let expected = [vec![value.clone(), value.clone()]];
        assert_eq!(buffered(&singleton, sql).unwrap(), expected);
        assert_eq!(stream(&singleton, sql, options(false)).unwrap(), expected);

        let population = Data::new(kind, vec![value, different]);
        let expected_error = buffered(&population, sql).unwrap_err();
        let actual = stream(&population, sql, options(false));
        assert!(
            matches!(actual, Err(StreamError::Query(error)) if error == expected_error),
            "{kind:?}"
        );
    }
}

#[test]
fn equal_signed_zero_and_fixed_point_keys_keep_the_first_representative() {
    for first in [0.0_f64, -0.0_f64] {
        let source = Data::new(
            ScalarType::Double,
            vec![
                Value::Double(Double::from_float(first)),
                Value::Double(Double::from_float(-first)),
            ],
        );
        let actual = stream(&source, "SELECT Min(N),Max(N) FROM T", options(false)).unwrap();
        for value in &actual[0] {
            let Value::Double(value) = value else {
                panic!("equal floating keys changed carrier");
            };
            assert_eq!(value.to_float().to_bits(), first.to_bits());
        }
        assert_eq!(
            actual,
            buffered(&source, "SELECT Min(N),Max(N) FROM T").unwrap()
        );
    }

    for first in [decimal(10, 1), decimal(100, 2)] {
        let second = if first == decimal(10, 1) {
            decimal(100, 2)
        } else {
            decimal(10, 1)
        };
        let source = Data::new(ScalarType::Decimal, vec![first.clone(), second]);
        let expected = [vec![first.clone(), first]];
        assert_eq!(
            stream(&source, "SELECT Min(N),Max(N) FROM T", options(false)).unwrap(),
            expected
        );
        assert_eq!(
            buffered(&source, "SELECT Min(N),Max(N) FROM T").unwrap(),
            expected
        );
    }
}

#[test]
fn mixed_binary_precision_uses_exact_key_widening_and_retains_selected_carriers() {
    for order in [[1, 2], [2, 1]] {
        let source = Data::new(ScalarType::Int32, order.map(Value::Int32).to_vec());
        let expression = "IIf(N=1,CSng(1),CDbl(1.00000001))";
        let sql = format!("SELECT Min({expression}),Max({expression}) FROM T");
        let expected = [vec![
            Value::Single(Single::from_float(1.0)),
            Value::Double(Double::from_float(1.00000001)),
        ]];
        assert_eq!(buffered(&source, &sql).unwrap(), expected);
        for declared in [false, true] {
            assert_eq!(stream(&source, &sql, options(declared)).unwrap(), expected);
        }
    }
}
