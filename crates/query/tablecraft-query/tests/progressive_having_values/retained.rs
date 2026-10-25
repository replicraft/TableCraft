use super::*;

struct Order;

impl Collation for Order {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right).reverse())
    }
}

static REVERSED: Order = Order;

struct Resolver<'a> {
    source: &'a Data,
    hits: Cell<usize>,
}

impl CollationResolver for Resolver<'_> {
    fn resolve(&self, id: TextCollationId) -> Result<&dyn Collation, QueryError> {
        assert!(self.source.eof.get());
        assert!(!self.source.active.get());
        self.hits.set(self.hits.get() + 1);
        if id.host_key() == 2 {
            Ok(&REVERSED)
        } else {
            Err(EvalError::CollationRequired.into())
        }
    }
}

#[test]
fn rejecting_having_never_activates_projection_only_text_provider() {
    let source = Data::fixture();
    let resolver = Resolver {
        source: &source,
        hits: Cell::new(0),
    };
    for predicate in ["False", "NULL", "Count(*)<0"] {
        let sql = format!("SELECT Min(A),Max(A) FROM T HAVING {predicate}");
        let actual = stream(
            &source,
            &sql,
            ExecutionOptions {
                collation_resolver: Some(&resolver),
                ..options(false)
            },
        )
        .unwrap();
        assert!(actual.is_empty());
        assert_eq!(resolver.hits.get(), 0);
    }
    let sql = "SELECT Min(A) FROM T HAVING True";
    let error = stream(
        &source,
        sql,
        ExecutionOptions {
            collation_resolver: Some(&resolver),
            ..options(false)
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        StreamError::Query(QueryError::Evaluation(EvalError::CollationRequired))
    ));
    assert!(resolver.hits.get() > 0);
}

#[test]
fn having_and_projection_string_populations_keep_late_complete_policy_independently() {
    let source = Data::new(&[(Some(2), 1), (Some(2), 2), (Some(2), 3)]);
    let resolver = Resolver {
        source: &source,
        hits: Cell::new(0),
    };
    let value = "IIf(N=3,B,N)&IIf(True,0,A)";
    let sql =
        format!("SELECT K,Min({value}),Max({value}) FROM T GROUP BY K HAVING Min({value})='20'");
    for declared in [false, true] {
        let actual = stream(
            &source,
            &sql,
            ExecutionOptions {
                collation_resolver: Some(&resolver),
                ..options(declared)
            },
        )
        .unwrap();
        let expected = vec![vec![
            Value::Int32(2),
            Value::Text("20".into()),
            Value::Text("10".into()),
        ]];
        assert_eq!(actual, expected);
    }
    assert!(resolver.hits.get() > 0);
}

#[test]
fn raw_presence_and_standard_errors_remain_observable_only_in_selected_consumers() {
    let source = Data::fixture();
    let sql = "SELECT K,Count(ChrW(55296)),Count(CVErr(N)),CLng(Min(CVErr(N))) \
               FROM T GROUP BY K HAVING Count(ChrW(55296))>=2";
    let expected = vec![
        vec![
            Value::Int32(2),
            Value::Int64(2),
            Value::Int64(2),
            Value::Int32(2),
        ],
        vec![
            Value::Int32(1),
            Value::Int64(2),
            Value::Int64(2),
            Value::Int32(1),
        ],
    ];
    for declared in [false, true] {
        assert_eq!(stream(&source, sql, options(declared)).unwrap(), expected);
    }
}
