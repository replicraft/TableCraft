use super::*;

struct Reversed;

impl Collation for Reversed {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right).reverse())
    }
}

static REVERSED: Reversed = Reversed;

struct Resolver<'a> {
    host: &'a Host,
    hits: Cell<usize>,
}

impl CollationResolver for Resolver<'_> {
    fn resolve(&self, id: TextCollationId) -> Result<&dyn Collation, QueryError> {
        assert!(
            !self.host.active.get(),
            "aggregate comparer activated inside cursor"
        );
        assert!(self.host.root.eof.get());
        self.hits.set(self.hits.get() + 1);
        if id.host_key() == 11 {
            Ok(&REVERSED)
        } else {
            Err(EvalError::CollationRequired.into())
        }
    }
}

#[test]
fn raw_error_cells_survive_two_projection_layers_until_explicit_observers() {
    let host = Host::new();
    let sql = "SELECT Count(R),Count(E),LenB(Min(R)),CLng(Min(E)) FROM \
               (SELECT R,E FROM \
               (SELECT ChrW(55296) AS R,CVErr(Id) AS E FROM T WHERE Id=10) X) Y";
    let actual = stream(&host, sql, options(false)).unwrap();
    let expected = vec![vec![
        Value::Int64(1),
        Value::Int64(1),
        Value::Int32(2),
        Value::Int32(10),
    ]];
    assert_eq!(actual, expected);
    let malformed = "SELECT R FROM (SELECT ChrW(55296) AS R FROM T WHERE Id=10) Q";
    assert!(matches!(
        stream(&host, malformed, options(false)),
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::UnrepresentableText
        )))
    ));
}

#[test]
fn complete_numeric_peer_filtering_survives_layers_and_outer_global_extrema() {
    let host = Host::new();
    let resolver = Resolver {
        host: &host,
        hits: Cell::new(0),
    };
    let sql = "SELECT Min(V),Max(V) FROM \
               (SELECT V FROM \
               (SELECT IIf(Id=12,B,Id)&IIf(True,0,A) AS V FROM T) X) Y";
    for declared in [false, true] {
        let actual = stream(
            &host,
            sql,
            ExecutionOptions {
                collation_resolver: Some(&resolver),
                ..options(declared)
            },
        )
        .unwrap();
        // Ordinal spellings 100,110,150, under the late B reverse comparer.
        let expected = vec![vec![Value::Text("150".into()), Value::Text("100".into())]];
        assert_eq!(actual, expected);
    }
    assert!(resolver.hits.get() > 0);
}

#[test]
fn grouped_outer_extrema_keep_separate_cohorts_and_first_group_order() {
    let host = Host::new();
    let resolver = Resolver {
        host: &host,
        hits: Cell::new(0),
    };
    let sql = "SELECT K,Min(V),Max(V) FROM \
               (SELECT K,V FROM \
               (SELECT CLng(Id Mod 2) AS K,IIf(Id=12,B,Id)&IIf(True,0,A) AS V FROM T) X) Y \
               GROUP BY K";
    let actual = stream(
        &host,
        sql,
        ExecutionOptions {
            collation_resolver: Some(&resolver),
            ..options(false)
        },
    )
    .unwrap();
    let expected = vec![
        vec![
            Value::Int32(0),
            Value::Text("150".into()),
            Value::Text("100".into()),
        ],
        vec![
            Value::Int32(1),
            Value::Text("110".into()),
            Value::Text("110".into()),
        ],
    ];
    assert_eq!(actual, expected);
    assert!(resolver.hits.get() > 0);
}

#[test]
fn every_preparation_and_execution_pair_retains_effective_numeric_profiles() {
    let host = Host::new();
    let sql = "SELECT VarType(V),VarType(V+V) FROM \
               (SELECT CDec(3)/CDbl(2) AS V FROM Nested WHERE Id=10) Q";
    for prepared_declared in [false, true] {
        let syntax = parse_query(sql, Default::default()).unwrap();
        let plan = prepare_with_options(
            syntax.query(),
            &host,
            PreparationOptions {
                declared_arithmetic: prepared_declared,
                ..Default::default()
            },
        )
        .unwrap();
        for declared in [false, true] {
            let mut actual = Vec::new();
            plan.execute_progressive(&host, &[], options(declared), &mut |row| {
                actual.push(row.to_vec());
                Ok::<_, ()>(())
            })
            .unwrap();
            let kind = if declared { 5 } else { 14 };
            let expected = vec![vec![Value::Int16(kind), Value::Int16(kind)]];
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn actual_string_conflicts_are_not_erased_by_projection_transport() {
    let host = Host::new();
    let resolver = Resolver {
        host: &host,
        hits: Cell::new(0),
    };
    let sql = "SELECT Min(V) FROM \
               (SELECT V FROM (SELECT IIf(Id=10,A,B) AS V FROM T) X) Y";
    let error = stream(
        &host,
        sql,
        ExecutionOptions {
            collation_resolver: Some(&resolver),
            ..options(false)
        },
    )
    .unwrap_err();
    assert!(host.root.eof.get());
    assert!(matches!(
        error,
        StreamError::Query(QueryError::CollationConflict)
    ));
    assert_eq!(resolver.hits.get(), 0);
}
