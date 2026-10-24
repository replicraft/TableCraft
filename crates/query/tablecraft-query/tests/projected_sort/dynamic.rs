use super::{MetadataOnly, source};
use tablecraft_core::values::Value;
use tablecraft_expressions::functions::math::random::RndState;
use tablecraft_query::{ExecutionOptions, QueryError, prepare};
use tablecraft_sql::{ParseLimits, parse_query};

#[test]
fn dynamic_eval_cannot_acquire_reusable_identity_from_equal_argument_syntax() {
    let source = MetadataOnly(source());
    for sql in [
        "SELECT DISTINCT Eval('Rnd()') FROM T ORDER BY Eval('Rnd()')",
        "SELECT DISTINCT Eval('Now()') FROM T ORDER BY Eval('Now()')",
        "SELECT DISTINCT Eval('Eval(''Rnd()'')') FROM T ORDER BY Eval('Eval(''Rnd()'')')",
        "SELECT DISTINCT Eval('Rnd()')+1 FROM T ORDER BY Eval('Rnd()')+1",
        "SELECT DISTINCT First(Eval('Rnd()')) FROM T ORDER BY First(Eval('Rnd()'))",
        "SELECT DISTINCT Eval('1+1') FROM T ORDER BY Eval('1+1')",
        "PARAMETERS P TEXT; SELECT DISTINCT Eval(P) FROM T ORDER BY Eval(P)",
    ] {
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source).err(),
            Some(QueryError::Unsupported),
            "{sql}"
        );
    }
}

fn single(value: &Value) -> f32 {
    let Value::Single(number) = value else {
        panic!("Rnd must retain its Single carrier");
    };
    number.to_float()
}

#[test]
fn selected_eval_aliases_and_ordinals_keep_one_dynamic_evaluation_per_row() {
    let source = source();
    for ordering in ["R", "1"] {
        let parsed = parse_query(
            &format!("SELECT DISTINCT Eval('Rnd()') AS R FROM T ORDER BY {ordering}"),
            ParseLimits::default(),
        )
        .unwrap();
        let state = RndState::new(123).unwrap();
        let reference = RndState::new(123).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    random: Some(&state),
                    ..Default::default()
                },
            )
            .unwrap();
        let mut expected: Vec<_> = (0..4)
            .map(|_| vec![reference.apply(None).unwrap()])
            .collect();
        expected.sort_by(|left, right| single(&left[0]).total_cmp(&single(&right[0])));
        assert_eq!(result.rows(), expected);
        assert_eq!(state.apply(None).unwrap(), reference.apply(None).unwrap());
    }
}

#[test]
fn ordinary_repeated_eval_ordering_keeps_independent_eager_evaluations() {
    let source = source();
    let parsed = parse_query(
        "SELECT Eval('Rnd()') FROM T ORDER BY Eval('Rnd()')",
        ParseLimits::default(),
    )
    .unwrap();
    let state = RndState::new(123).unwrap();
    let reference = RndState::new(123).unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                random: Some(&state),
                ..Default::default()
            },
        )
        .unwrap();
    let mut expected: Vec<_> = (0..4)
        .map(|_| {
            let value = reference.apply(None).unwrap();
            let key = reference.apply(None).unwrap();
            (value, single(&key))
        })
        .collect();
    expected.sort_by(|left, right| left.1.total_cmp(&right.1));
    let expected: Vec<_> = expected.into_iter().map(|(value, _)| vec![value]).collect();
    assert_eq!(result.rows(), expected);
    assert_eq!(state.apply(None).unwrap(), reference.apply(None).unwrap());
}

#[test]
fn pure_eval_still_executes_with_explicit_selected_output_ordering() {
    let source = source();
    let parsed = parse_query(
        "SELECT DISTINCT Eval('1+1') AS R FROM T ORDER BY R",
        ParseLimits::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        [vec![Value::Int64(2)]]
    );
}
