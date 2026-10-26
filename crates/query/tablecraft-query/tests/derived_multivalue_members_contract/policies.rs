use super::support::*;
use std::{
    cell::{Cell, RefCell},
    cmp::Ordering,
};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryValue, prepare,
    source::{CollationResolver, TextCollationId},
};

struct Comparer {
    reverse: bool,
    all_equal: bool,
    calls: Cell<usize>,
}

impl Comparer {
    fn new(reverse: bool, all_equal: bool) -> Self {
        Self {
            reverse,
            all_equal,
            calls: Cell::new(0),
        }
    }
}

impl Collation for Comparer {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        self.calls.set(self.calls.get() + 1);
        Ok(if self.all_equal {
            Ordering::Equal
        } else if self.reverse {
            left.cmp(right).reverse()
        } else {
            left.cmp(right)
        })
    }
}

struct Resolver<'a> {
    active: &'a Cell<bool>,
    hits: RefCell<Vec<u64>>,
    ordinal: Comparer,
    reverse: Comparer,
}

impl<'a> Resolver<'a> {
    fn new(active: &'a Cell<bool>) -> Self {
        Self {
            active,
            hits: RefCell::new(vec![]),
            ordinal: Comparer::new(false, false),
            reverse: Comparer::new(true, false),
        }
    }
}

impl CollationResolver for Resolver<'_> {
    fn resolve(&self, policy: TextCollationId) -> Result<&dyn Collation, QueryError> {
        assert!(
            !self.active.get(),
            "retained-list consumers run after source EOF"
        );
        self.hits.borrow_mut().push(policy.host_key());
        match policy.host_key() {
            11 => Ok(&self.ordinal),
            22 => Ok(&self.reverse),
            _ => Err(QueryError::Evaluation(EvalError::CollationRequired)),
        }
    }
}

fn scalar_text(value: &str) -> Vec<QueryValue> {
    vec![QueryValue::Scalar(ExpressionValue::Value(Value::Text(
        value.into(),
    )))]
}

#[test]
fn retained_element_policies_survive_reordering_without_activating_bare_projection() {
    let mut data = text_fixture();
    data.definitions.push((
        "Words",
        syntax("SELECT Id,B AS RightWords,A AS LeftWords FROM L"),
    ));
    let bare = prepare(&syntax("SELECT LeftWords,RightWords FROM Words"), &data).unwrap();
    assert!(data.scans.borrow().is_empty());
    let result = bare
        .execute_collections(&data, &[], Default::default())
        .unwrap();
    assert_eq!(result.rows().len(), 1);

    let resolver = Resolver::new(&data.active);
    let default = Comparer::new(false, true);
    for (name, wanted, id) in [
        ("LeftWords", ["A", "a"], 11),
        ("RightWords", ["z", "a"], 22),
    ] {
        let sql = format!("SELECT {name}.Value FROM Words ORDER BY {name}.Value");
        let plan = prepare(&syntax(&sql), &data).unwrap();
        let before = resolver.hits.borrow().len();
        let result = plan
            .execute_collections(
                &data,
                &[],
                ExecutionOptions {
                    collation: Some(&default),
                    collation_resolver: Some(&resolver),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows(), wanted.map(scalar_text));
        assert!(
            resolver.hits.borrow()[before..]
                .iter()
                .all(|actual| *actual == id)
        );
        assert!(resolver.hits.borrow().len() > before);
    }
    assert_eq!(default.calls.get(), 0);
}

#[test]
fn retained_unknown_policy_and_conflict_do_not_fall_back_to_a_global_comparer() {
    let mut data = text_fixture();
    let resolver = Resolver::new(&data.active);
    let default = Comparer::new(false, true);
    let sql = "SELECT Q.X.Value FROM (SELECT A AS X,B AS Y FROM L) Q \
               WHERE Q.X.Value=Q.Y.Value";
    let plan = prepare(&syntax(sql), &data).unwrap();
    let result = plan.execute_collections(
        &data,
        &[],
        ExecutionOptions {
            collation: Some(&default),
            collation_resolver: Some(&resolver),
            ..Default::default()
        },
    );
    assert_eq!(result.err(), Some(QueryError::CollationConflict));
    assert!(resolver.hits.borrow().is_empty());
    assert_eq!(default.calls.get(), 0);

    drop(resolver);
    data.policies[0] = Some(TextCollationId::from_host(999));
    let resolver = Resolver::new(&data.active);
    let sql = "SELECT Q.X.Value FROM (SELECT A AS X FROM L) Q WHERE Q.X.Value='a'";
    let plan = prepare(&syntax(sql), &data).unwrap();
    let result = plan.execute_collections(
        &data,
        &[],
        ExecutionOptions {
            collation: Some(&default),
            collation_resolver: Some(&resolver),
            ..Default::default()
        },
    );
    assert_eq!(
        result.err(),
        Some(QueryError::Evaluation(EvalError::CollationRequired))
    );
    assert_eq!(*resolver.hits.borrow(), [999]);
    assert_eq!(default.calls.get(), 0);
}
