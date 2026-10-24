use super::*;
use tablecraft_core::values::{ExtendedDateTime, Guid, LegacyDateTime};

#[test]
fn nonnumeric_operands_do_not_acquire_serial_or_byte_conversions() {
    for (kind, value) in [
        (
            ScalarType::DateTime,
            Value::DateTime(LegacyDateTime::from_ole_days(-1.25).unwrap()),
        ),
        (
            ScalarType::DateTimeExtended,
            Value::DateTimeExtended(ExtendedDateTime::new(2024, 1, 1, 0, 0, 0, 0).unwrap()),
        ),
        (ScalarType::Text, Value::Text("1.25".into())),
        (ScalarType::Guid, Value::Guid(Guid::from_bytes([0; 16]))),
        (ScalarType::Binary, Value::Binary(vec![1])),
    ] {
        for values in [vec![value.clone()], vec![Value::Null, value.clone()]] {
            let data = source(kind, values);
            for function in ["Var", "VarP", "StDev", "StDevP"] {
                assert_eq!(
                    run(&format!("SELECT {function}(X) FROM T"), &data).err(),
                    Some(EvalError::TypeMismatch.into()),
                    "{function}({kind:?})",
                );
            }
        }

        // Empty/all-NULL groups retain their population rules, without looking
        // at a source carrier as proof that a non-NULL operand was evaluated.
        for values in [vec![], vec![Value::Null]] {
            assert_eq!(
                run(
                    "SELECT Var(X),VarP(X),StDev(X),StDevP(X) FROM T",
                    &source(kind, values)
                )
                .unwrap()
                .rows(),
                [vec![Value::Null; 4]],
            );
        }
    }
}

#[test]
fn explicit_date_serial_conversion_remains_available_to_statistics_and_domains() {
    let data = source(
        ScalarType::DateTime,
        vec![
            Value::DateTime(LegacyDateTime::from_ole_days(-1.25).unwrap()),
            Value::Null,
            Value::DateTime(LegacyDateTime::from_ole_days(2.75).unwrap()),
        ],
    );
    for (function, domain, expected) in [
        ("Var", "DVar", 8.0_f64),
        ("VarP", "DVarP", 4.0),
        ("StDev", "DStDev", 8.0_f64.sqrt()),
        ("StDevP", "DStDevP", 2.0),
    ] {
        let sql = format!(
            "SELECT {function}(CDbl(X)),{domain}('CDbl(X)','T','X IS NOT NULL') FROM T WHERE X IS NOT NULL"
        );
        let result = run(&sql, &data).unwrap();
        for value in &result.rows()[0] {
            near(float(value), expected);
        }

        let unfiltered = format!("SELECT {function}(CDbl(X)) FROM T");
        assert_eq!(
            run(&unfiltered, &data).err(),
            Some(EvalError::InvalidNullConversion.into())
        );

        let sql = format!("SELECT {domain}('X','T')");
        assert_eq!(run(&sql, &data).err(), Some(EvalError::TypeMismatch.into()));
    }
}
