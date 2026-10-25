use super::*;

struct Units<'a> {
    source: &'a Input,
    calls: Cell<usize>,
}

impl Collation for Units<'_> {
    fn compare(&self, _left: &str, _right: &str) -> Result<Ordering, EvalError> {
        panic!("explicit raw-unit provider must use compare_utf16");
    }

    fn string_comparison_profile(&self) -> StringComparisonProfile {
        StringComparisonProfile::Utf16
    }

    fn compare_utf16(&self, left: &[u16], right: &[u16]) -> Result<Ordering, EvalError> {
        assert!(self.source.eof.get());
        assert!(!self.source.active.get());
        self.calls.set(self.calls.get() + 1);
        Ok(left.cmp(right))
    }
}

fn buffered(
    sql: &str,
    source: &Input,
    options: ExecutionOptions<'_>,
) -> Result<Vec<Vec<Value>>, QueryError> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare_with_options(syntax.query(), source, Default::default())?;
    plan.execute(source, &[], options)
        .map(|result| result.rows().to_vec())
}

#[test]
fn singleton_raw_and_odd_keys_need_no_comparison_but_strict_key_publication_refuses() {
    for (expression, input, width) in [("ChrW(Id)", 55296, 2), ("ChrB(Id)", 65, 1)] {
        let mut source = Input::new(
            &[("Id", ScalarType::Int32)],
            vec![vec![Value::Int32(input)]],
        );
        source.allow_buffered = true;
        let relation = format!("(SELECT {expression} AS S FROM T) Q");
        let observed = format!("SELECT LenB(S),Count(*) FROM {relation} GROUP BY S");
        let expected = vec![vec![Value::Int32(width), Value::Int64(1)]];
        assert_eq!(
            buffered(&observed, &source, Default::default()).unwrap(),
            expected
        );
        assert_eq!(
            stream(&observed, &source, false, Default::default()).unwrap(),
            expected
        );

        let published = format!("SELECT S,Count(*) FROM {relation} GROUP BY S");
        assert_eq!(
            buffered(&published, &source, Default::default()),
            Err(QueryError::Evaluation(EvalError::UnrepresentableText))
        );
        assert!(matches!(
            stream(&published, &source, false, Default::default()),
            Err(StreamError::Query(QueryError::Evaluation(
                EvalError::UnrepresentableText
            )))
        ));
    }
}

#[test]
fn repeated_raw_keys_keep_existing_missing_comparer_and_unicode_error_priorities() {
    for (expression, input, expected_error) in [
        ("ChrW(Id)", 55296, EvalError::UnrepresentableText),
        ("ChrB(Id)", 65, EvalError::UnrepresentableText),
        ("ChrW(Id)", 65, EvalError::CollationRequired),
    ] {
        let mut source = Input::new(
            &[("Id", ScalarType::Int32)],
            vec![vec![Value::Int32(input)]; 2],
        );
        source.allow_buffered = true;
        let sql = format!("SELECT Count(*) FROM (SELECT {expression} AS S FROM T) Q GROUP BY S");
        assert_eq!(
            buffered(&sql, &source, Default::default()),
            Err(QueryError::Evaluation(expected_error))
        );
        assert!(matches!(
            stream(&sql, &source, false, Default::default()),
            Err(StreamError::Query(QueryError::Evaluation(error))) if error == expected_error
        ));
    }
}

#[test]
fn explicit_utf16_provider_compares_unpaired_units_but_never_pads_odd_bytes() {
    for (expression, input, expected) in [
        (
            "ChrW(Id)",
            55296,
            Some(vec![vec![Value::Int32(2), Value::Int64(2)]]),
        ),
        ("ChrB(Id)", 65, None),
    ] {
        let mut source = Input::new(
            &[("Id", ScalarType::Int32)],
            vec![vec![Value::Int32(input)]; 2],
        );
        source.allow_buffered = true;
        let provider = Units {
            source: &source,
            calls: Cell::new(0),
        };
        let sql =
            format!("SELECT LenB(S),Count(*) FROM (SELECT {expression} AS S FROM T) Q GROUP BY S");
        let options = ExecutionOptions {
            collation: Some(&provider),
            ..Default::default()
        };
        let old = buffered(&sql, &source, options);
        let progressive = stream(&sql, &source, false, options);
        if let Some(expected) = expected {
            assert_eq!(old.unwrap(), expected);
            assert_eq!(progressive.unwrap(), expected);
            assert!(provider.calls.get() > 0);
        } else {
            assert_eq!(
                old,
                Err(QueryError::Evaluation(EvalError::UnrepresentableText))
            );
            assert!(matches!(
                progressive,
                Err(StreamError::Query(QueryError::Evaluation(
                    EvalError::UnrepresentableText
                )))
            ));
            assert_eq!(provider.calls.get(), 0);
        }
    }
}

#[test]
fn raw_and_null_key_membership_does_not_require_a_string_comparison() {
    let mut source = Input::new(
        &[("Id", ScalarType::Int32)],
        vec![vec![Value::Int32(0)], vec![Value::Int32(1)]],
    );
    source.allow_buffered = true;
    let sql = "SELECT Count(*) FROM (SELECT IIf(Id=0,NULL,ChrW(55296)) AS S FROM T) Q GROUP BY S";
    let expected = vec![vec![Value::Int64(1)], vec![Value::Int64(1)]];
    assert_eq!(
        buffered(sql, &source, Default::default()).unwrap(),
        expected
    );
    assert_eq!(
        stream(sql, &source, false, Default::default()).unwrap(),
        expected
    );
}
