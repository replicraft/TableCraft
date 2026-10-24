use super::*;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_query::source::{CollationResolver, TextCollationId, TextEqualityKey};

const POLICY: TextCollationId = TextCollationId::from_host(7);
const POLICIES: [Option<TextCollationId>; 2] = [Some(POLICY), None];

struct TextHints {
    inner: Restricted,
    enabled: bool,
    tagged: Cell<bool>,
}

fn source(ignore: bool) -> TextHints {
    TextHints {
        inner: Restricted {
            source: MemorySource::new(vec![table(
                "T",
                &[("X", ScalarType::Text), ("Y", ScalarType::Int32)],
                vec![
                    vec![Value::Null, Value::Int32(1)],
                    vec![Value::Text(String::new()), Value::Int32(2)],
                    vec![Value::Text("a".into()), Value::Int32(3)],
                    vec![Value::Text("ab".into()), Value::Int32(4)],
                    vec![Value::Text("á".into()), Value::Int32(5)],
                ],
            )])
            .unwrap(),
            seen: Cell::new(None),
            scans: Cell::new(0),
            ignore,
        },
        enabled: true,
        tagged: Cell::new(true),
    }
}

impl Source for TextHints {
    fn text_equality_hints(&self, _table: &str) -> Result<bool, QueryError> {
        Ok(self.enabled)
    }

    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.inner.columns(name)
    }

    fn text_collations(&self, _name: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        Ok(if self.tagged.get() { &POLICIES } else { &[] })
    }

    fn scan(
        &self,
        name: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.inner.scan(name, visitor)
    }

    fn scan_constrained(
        &self,
        name: &str,
        constraint: ScanConstraint,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.inner.seen.set(Some(constraint));
        let ScanConstraint::TextEqual { column, key } = constraint else {
            return self.inner.scan_constrained(name, constraint, visitor);
        };
        assert_eq!(key.policy(), POLICY);
        self.inner.source.scan(name, &mut |row| {
            let retained = matches!(&row[column], Value::Text(value) if value.as_bytes().starts_with(key.as_bytes()));
            if self.inner.ignore || retained {
                visitor(row)?;
            }
            Ok(())
        })
    }
}

struct Resolver {
    calls: Cell<usize>,
    failure: Option<QueryError>,
    wrong_policy: bool,
}

impl Resolver {
    fn new() -> Self {
        Self {
            calls: Cell::new(0),
            failure: None,
            wrong_policy: false,
        }
    }
}

impl CollationResolver for Resolver {
    fn resolve(&self, policy: TextCollationId) -> Result<&dyn Collation, QueryError> {
        assert_eq!(policy, POLICY);
        Ok(&Ordinal)
    }

    fn equality_key(
        &self,
        policy: TextCollationId,
        literal: &str,
    ) -> Result<Option<TextEqualityKey>, QueryError> {
        self.calls.set(self.calls.get() + 1);
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let policy = if self.wrong_policy {
            TextCollationId::from_host(8)
        } else {
            policy
        };
        TextEqualityKey::from_host(policy, &literal.as_bytes()[..literal.len().min(1)]).map(Some)
    }
}

fn execute(
    sql: &str,
    source: &TextHints,
    resolver: &dyn CollationResolver,
) -> Result<QueryResult, QueryError> {
    let query = parse_query(sql, Default::default()).unwrap();
    prepare(query.query(), source)?.execute(
        source,
        &[],
        ExecutionOptions {
            collation: Some(&Ordinal),
            collation_resolver: Some(resolver),
            ..Default::default()
        },
    )
}

struct ComparerOnly;

impl CollationResolver for ComparerOnly {
    fn resolve(&self, _policy: TextCollationId) -> Result<&dyn Collation, QueryError> {
        Ok(&Ordinal)
    }
}

#[test]
fn text_keys_require_two_participating_hosts_and_keep_the_residual_filter() {
    for ignore in [false, true] {
        for sql in ["SELECT Y FROM T WHERE X='a'", "SELECT Y FROM T WHERE 'a'=X"] {
            let source = source(ignore);
            let resolver = Resolver::new();
            let query = parse_query(sql, Default::default()).unwrap();
            let plan = prepare(query.query(), &source).unwrap();
            assert_eq!(resolver.calls.get(), 0);
            assert_eq!(source.inner.scans.get(), 0);
            let result = plan
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        collation_resolver: Some(&resolver),
                        ..Default::default()
                    },
                )
                .unwrap();
            assert_eq!(result.rows(), [vec![Value::Int32(3)]]);
            assert_eq!(resolver.calls.get(), 1);
            let Some(ScanConstraint::TextEqual { column, key }) = source.inner.seen.get() else {
                panic!("missing equality hint");
            };
            assert_eq!(column, 0);
            assert_eq!(key.as_bytes(), b"a");
        }
    }

    for (enabled, tagged) in [(false, true), (true, false)] {
        let mut source = source(false);
        source.enabled = enabled;
        source.tagged.set(tagged);
        let resolver = Resolver::new();
        assert_eq!(
            execute("SELECT Y FROM T WHERE X='a'", &source, &resolver)
                .unwrap()
                .rows(),
            [vec![Value::Int32(3)]]
        );
        assert_eq!(resolver.calls.get(), 0);
        assert_eq!(source.inner.seen.get(), None);
    }
}

#[test]
fn existing_comparers_and_global_overrides_do_not_issue_implicit_keys() {
    let source = source(false);
    let result = execute("SELECT Y FROM T WHERE X='a'", &source, &ComparerOnly).unwrap();
    assert_eq!(result.rows(), [vec![Value::Int32(3)]]);
    assert_eq!(source.inner.seen.get(), None);

    let query = parse_query("SELECT Y FROM T WHERE X='a'", Default::default()).unwrap();
    let result = prepare(query.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&Ordinal),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.rows(), [vec![Value::Int32(3)]]);
    assert_eq!(source.inner.seen.get(), None);
}

#[test]
fn only_direct_top_level_equalities_issue_text_keys() {
    for filter in [
        "X<'b'",
        "Left(X,1)='a'",
        "X=CStr('a')",
        "X='a' OR Y=2",
        "X='a' AND Y=3",
    ] {
        let source = source(false);
        let resolver = Resolver::new();
        execute(
            &format!("SELECT Y FROM T WHERE {filter}"),
            &source,
            &resolver,
        )
        .unwrap();
        assert_eq!(resolver.calls.get(), 0, "{filter}");
        assert_eq!(source.inner.seen.get(), None, "{filter}");
    }
    let source = source(false);
    let resolver = Resolver::new();
    assert_eq!(
        execute("SELECT Y FROM T WHERE X='a' AND 1/0=0", &source, &resolver).err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    assert_eq!(resolver.calls.get(), 0);
}

#[test]
fn prepared_text_parameters_reissue_keys_and_null_does_not_become_a_key() {
    let source = source(false);
    let resolver = Resolver::new();
    let query = parse_query(
        "PARAMETERS P TEXT; SELECT Y FROM T WHERE X=P",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (text, id) in [("a", 3), ("ab", 4), ("á", 5)] {
        let result = plan
            .execute(
                &source,
                &[("P", Value::Text(text.into()))],
                ExecutionOptions {
                    collation_resolver: Some(&resolver),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows(), [vec![Value::Int32(id)]]);
    }
    source.inner.seen.set(None);
    let result = plan
        .execute(
            &source,
            &[("P", Value::Null)],
            ExecutionOptions {
                collation_resolver: Some(&resolver),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(result.rows().is_empty());
    assert_eq!(resolver.calls.get(), 3);
    assert_eq!(source.inner.seen.get(), None);
}

#[test]
fn bad_namespaces_and_callback_failures_publish_no_source_rows() {
    for (wrong_policy, error, expected) in [
        (true, None, QueryError::Source),
        (false, Some(QueryError::Cancelled), QueryError::Cancelled),
        (
            false,
            Some(QueryError::Evaluation(EvalError::Cancelled)),
            QueryError::Cancelled,
        ),
    ] {
        let source = source(false);
        let mut resolver = Resolver::new();
        resolver.wrong_policy = wrong_policy;
        resolver.failure = error;
        assert_eq!(
            execute("SELECT Y FROM T WHERE X='a'", &source, &resolver).err(),
            Some(expected)
        );
        assert_eq!(source.inner.scans.get(), 0);
        assert_eq!(source.inner.seen.get(), None);
    }
    let source = source(false);
    let mut resolver = Resolver::new();
    let limit = ResourceBudget::new(0, 0, 0)
        .charge(Resource::Steps, 1)
        .unwrap_err();
    resolver.failure = Some(QueryError::Evaluation(EvalError::Limit(limit)));
    assert_eq!(
        execute("SELECT Y FROM T WHERE X='a'", &source, &resolver).err(),
        Some(QueryError::Limit(limit))
    );
    assert_eq!(source.inner.seen.get(), None);

    assert_eq!(
        TextEqualityKey::from_host(POLICY, &[0; 33]),
        Err(QueryError::Source)
    );
    let full = TextEqualityKey::from_host(POLICY, &[7; 32]).unwrap();
    assert_eq!(full.as_bytes(), &[7; 32]);
    assert_eq!(
        TextEqualityKey::from_host(POLICY, &[]).unwrap().as_bytes(),
        &[]
    );
}

#[test]
fn changed_text_metadata_is_rejected_before_comparer_key_activation() {
    let source = source(false);
    let resolver = Resolver::new();
    let query = parse_query("SELECT Y FROM T WHERE X='a'", Default::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    source.tagged.set(false);
    let result = plan.execute(
        &source,
        &[],
        ExecutionOptions {
            collation_resolver: Some(&resolver),
            ..Default::default()
        },
    );
    assert_eq!(result.err(), Some(QueryError::SchemaChanged));
    assert_eq!(resolver.calls.get(), 0);
    assert_eq!(source.inner.seen.get(), None);
}

#[test]
fn text_literals_outside_the_consumer_profile_do_not_activate_hint_providers() {
    let source = source(false);
    let resolver = Resolver::new();
    let query = parse_query("SELECT Y FROM T WHERE X='ab'", Default::default()).unwrap();
    let result = prepare(query.query(), &source).unwrap().execute(
        &source,
        &[],
        ExecutionOptions {
            max_text_bytes: Some(1),
            collation_resolver: Some(&resolver),
            ..Default::default()
        },
    );
    assert_eq!(
        result.err(),
        Some(QueryError::Evaluation(EvalError::TextLimit))
    );
    assert_eq!(resolver.calls.get(), 0);
    assert_eq!(source.inner.seen.get(), None);
}
