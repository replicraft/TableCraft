use super::*;
use std::cmp::Ordering;
use tablecraft_expressions::comparison::Collation;

#[test]
fn raw_and_error_singleton_extrema_remain_retained_for_outer_observers() {
    let source = Input::new(&[("Id", ScalarType::Int32)], vec![vec![Value::Int32(1)]]);
    let (_, actual) = run(
        "SELECT LenB(R),CLng(AscW(R)),CLng(E),VarType(E) FROM (SELECT Min(ChrW(55296)) AS R,Min(CVErr(7)) AS E FROM T) Q",
        &source, &[], false, false,
    ).unwrap();
    assert_eq!(
        actual,
        [vec![
            Value::Int32(2),
            Value::Int32(-10240),
            Value::Int32(7),
            Value::Int16(10)
        ]]
    );
    assert!(matches!(
        run(
            "SELECT R FROM (SELECT Min(ChrW(55296)) AS R FROM T) Q",
            &source,
            &[],
            false,
            false
        ),
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::UnrepresentableText
        )))
    ));
}

struct AsciiFold;

impl Collation for AsciiFold {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.to_ascii_uppercase().cmp(&right.to_ascii_uppercase()))
    }
}

#[test]
fn inner_text_groups_feed_outer_numeric_kernels_using_final_host_equivalence() {
    let source = Input::new(
        &[("Label", ScalarType::Text)],
        [Some("b"), Some("A"), Some("B"), None, Some("a"), None]
            .into_iter()
            .map(|text| vec![text.map_or(Value::Null, |text| Value::Text(text.into()))])
            .collect(),
    );
    let sql = "SELECT Count(N),Sum(N) FROM (SELECT Label,Count(*) AS N FROM T GROUP BY Label) Q WHERE Label IS NOT NULL";
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare_with_options(syntax.query(), &source, Default::default()).unwrap();
    let mut rows = Vec::new();
    let summary = plan
        .execute_progressive(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&AsciiFold),
                ..Default::default()
            },
            &mut |row| {
                rows.push(row.to_vec());
                Ok::<_, ()>(())
            },
        )
        .unwrap();
    // ASCII B/b and A/a each contain two inputs; the NULL group is filtered.
    assert_eq!(rows, [vec![Value::Int64(2), Value::Int64(4)]]);
    assert_eq!(summary.scanned_rows, 6);
    assert_eq!(summary.emitted_rows, 1);
}
