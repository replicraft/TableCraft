use super::*;
use std::cmp::Ordering;
use tablecraft_expressions::comparison::Collation;
use tablecraft_query::source::CollationResolver;

struct Order(bool);

impl Collation for Order {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        let order = left.cmp(right);
        Ok(if self.0 { order.reverse() } else { order })
    }
}

static ORDINAL: Order = Order(false);
static REVERSED: Order = Order(true);

struct Resolver<'a> {
    source: &'a Data,
    hits: Cell<usize>,
}

impl CollationResolver for Resolver<'_> {
    fn resolve(&self, id: TextCollationId) -> Result<&dyn Collation, QueryError> {
        assert!(
            self.source.eof.get(),
            "provider activated before checked EOF"
        );
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
fn late_actual_text_policy_selects_the_whole_population_after_checked_eof() {
    let mut source = Data::new(ScalarType::Int32, Vec::new());
    source.columns = [
        ("Id", ScalarType::Int32),
        ("A", ScalarType::Text),
        ("B", ScalarType::Text),
    ]
    .map(|(name, kind)| Column::new(name.into(), kind, true).unwrap())
    .to_vec();
    source.rows = (1..=3)
        .map(|id| {
            vec![
                Value::Int32(id),
                Value::Text("unused".into()),
                Value::Text("15".into()),
            ]
        })
        .collect();
    source.policies = vec![
        None,
        Some(TextCollationId::from_host(1)),
        Some(TextCollationId::from_host(2)),
    ];
    let expression = "IIf(Id=3,B,Id)&IIf(True,0,A)";
    let sql = format!("SELECT Min({expression}),Max({expression}) FROM T");
    let expected = [vec![Value::Text("20".into()), Value::Text("10".into())]];

    for declared in [false, true] {
        let resolver = Resolver {
            source: &source,
            hits: Cell::new(0),
        };
        let actual = stream(
            &source,
            &sql,
            ExecutionOptions {
                collation: Some(&ORDINAL),
                collation_resolver: Some(&resolver),
                ..options(declared)
            },
        )
        .unwrap();
        // Complete values are 10, 20 and 150; reverse ordinal selects 20 and 10.
        assert_eq!(actual, expected);
        assert!(resolver.hits.get() > 0);
    }
}

#[test]
fn a_required_text_singleton_does_not_activate_a_provider_without_comparison() {
    let mut source = Data::new(ScalarType::Text, vec![Value::Text("only".into())]);
    source.policies = vec![Some(TextCollationId::from_host(999))];
    let resolver = Resolver {
        source: &source,
        hits: Cell::new(0),
    };
    let actual = stream(
        &source,
        "SELECT Len(Min(N)),Len(Max(N)) FROM T",
        ExecutionOptions {
            collation_resolver: Some(&resolver),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(actual, [vec![Value::Int32(4), Value::Int32(4)]]);
    assert_eq!(resolver.hits.get(), 0);
}
