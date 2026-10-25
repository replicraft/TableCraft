use super::support::{Numbers, PERMUTATIONS, number, options, run};
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, QueryError, QueryLimits, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn scalar_predicates_keep_pairwise_narrowing_and_static_union_keeps_double_promotion() {
    let source = MemorySource::default();
    for declared in [false, true] {
        let scalar = run("SELECT CSng(1)=CDbl(1.00000001)", &source, declared).unwrap();
        assert_eq!(scalar.rows(), &[vec![Value::Boolean(true)]]);

        let union = run(
            "SELECT CSng(1) AS K UNION SELECT CDbl(1.00000001) ORDER BY K",
            &source,
            declared,
        )
        .unwrap();
        assert_eq!(union.rows().len(), 2);
        assert!(
            union
                .rows()
                .iter()
                .all(|row| matches!(row[0], Value::Double(_)))
        );
        assert_eq!(number(&union.rows()[0][0]), 1.0);
        assert_eq!(number(&union.rows()[1][0]), 1.00000001);
    }
}

#[test]
fn signed_zero_keys_alias_without_rewriting_retained_carriers() {
    for order in PERMUTATIONS {
        let source = Numbers::new(order, "-0.0", "0.0", "-0.0");
        for declared in [false, true] {
            let grouped = run(
                "SELECT K,Count(*) FROM Numbers GROUP BY K",
                &source,
                declared,
            )
            .unwrap();
            assert_eq!(grouped.rows().len(), 1);
            assert_eq!(grouped.rows()[0][1], Value::Int64(3));
            let top = run("SELECT TOP 1 K FROM Numbers ORDER BY K", &source, declared).unwrap();
            assert_eq!(top.rows().len(), 3);
            assert_eq!(
                top.rows()
                    .iter()
                    .filter(|row| matches!(row[0], Value::Single(_)))
                    .count(),
                1
            );
        }
    }
}

#[test]
fn strict_key_family_refusals_remain_outside_float_widening() {
    let source = MemorySource::default();
    for (left, right, error) in [
        (
            "CDec(1)",
            "CDbl(1)",
            EvalError::UnsupportedNumericCombination,
        ),
        (
            "CCur(1)",
            "CSng(1)",
            EvalError::UnsupportedNumericCombination,
        ),
        (
            "CLngLng(9007199254740993)",
            "CDbl(9007199254740992)",
            EvalError::PrecisionLoss,
        ),
        ("CDate(1)", "CDbl(1)", EvalError::TypeMismatch),
        ("'1'", "CDbl(1)", EvalError::TypeMismatch),
    ] {
        for declared in [false, true] {
            let sql = format!(
                "SELECT DISTINCT IIf(Id=1,{left},{right}) AS K FROM (SELECT 1 AS Id UNION ALL SELECT 2) X"
            );
            assert_eq!(
                run(&sql, &source, declared).err(),
                Some(QueryError::Evaluation(error)),
                "{sql}"
            );
        }
    }
}

#[test]
fn eager_failures_and_consumable_limits_do_not_publish_a_key_prefix() {
    let source = Numbers::normal([1, 2, 3]);
    let sql = "SELECT DISTINCT IIf(Id=1,CSng(1),CDbl(1)/0) FROM Inputs";
    assert!(run(sql, &source, false).is_err());

    let syntax = parse_query("SELECT K FROM Numbers ORDER BY K", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let cancelled = || true;
    let cancelled_options = ExecutionOptions {
        cancelled: Some(&cancelled),
        ..options(false)
    };
    assert!(matches!(
        plan.execute(&source, &[], cancelled_options),
        Err(QueryError::Cancelled)
    ));

    let limited = ExecutionOptions {
        limits: QueryLimits {
            steps: 1,
            ..Default::default()
        },
        ..options(false)
    };
    assert!(matches!(
        plan.execute(&source, &[], limited),
        Err(QueryError::Limit(_))
    ));
}
