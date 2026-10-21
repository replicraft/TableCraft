use std::{
    cell::{Cell, RefCell},
    cmp::Ordering,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
pub(super) use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, comparison::Collation};
pub(super) use tablecraft_query::{
    ExecutionOptions, QueryError, Source, prepare,
    source::{CollationResolver, TextCollationId},
};
use tablecraft_schema::{Column, ScalarType};
pub(super) use tablecraft_sql::{ParseLimits, parse_query};

pub(super) struct Comparer(pub u8);
impl Collation for Comparer {
    fn compare(&self, a: &str, b: &str) -> Result<Ordering, EvalError> {
        Ok(match self.0 {
            1 => a
                .bytes()
                .map(|c| c.to_ascii_lowercase())
                .cmp(b.bytes().map(|c| c.to_ascii_lowercase())),
            2 => a.cmp(b).reverse(),
            _ => a.cmp(b),
        })
    }
}
pub(super) static DEFAULT: Comparer = Comparer(0);
static FOLD: Comparer = Comparer(1);
static REVERSE: Comparer = Comparer(2);
pub(super) struct Resolver {
    pub hits: RefCell<Vec<u64>>,
    pub budget: RefCell<ResourceBudget>,
    pub cancel_on_resolve: Cell<bool>,
    pub cancelled: Cell<bool>,
}
impl Resolver {
    pub fn new() -> Self {
        Self {
            hits: RefCell::new(vec![]),
            budget: RefCell::new(ResourceBudget::new(0, 0, 10000)),
            cancel_on_resolve: Cell::new(false),
            cancelled: Cell::new(false),
        }
    }
}
impl CollationResolver for Resolver {
    fn resolve(&self, id: TextCollationId) -> Result<&dyn Collation, QueryError> {
        self.hits.borrow_mut().push(id.host_key());
        self.budget.borrow_mut().charge(Resource::Steps, 1)?;
        if self.cancel_on_resolve.get() {
            self.cancelled.set(true);
        }
        match id.host_key() {
            1 => Ok(&FOLD),
            2 => Ok(&REVERSE),
            3 => Ok(&DEFAULT),
            _ => Err(EvalError::CollationRequired.into()),
        }
    }
}
pub(super) struct Fixture {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Value>>,
    pub policies: Vec<Option<TextCollationId>>,
    pub scans: Cell<usize>,
    pub named: tablecraft_sql::query::Query,
}
impl Fixture {
    pub fn new() -> Self {
        Self {
            columns: [
                ("Id", ScalarType::Int32),
                ("A", ScalarType::Text),
                ("B", ScalarType::Text),
                ("K", ScalarType::Text),
                ("U", ScalarType::Text),
            ]
            .map(|(n, t)| Column::new(n.into(), t, true).unwrap())
            .to_vec(),
            rows: [
                ("a", "a", "a"),
                ("A", "A", "b"),
                ("b", "a", "c"),
                ("b", "B", "d"),
            ]
            .into_iter()
            .enumerate()
            .map(|(i, (a, b, k))| {
                vec![
                    Value::Int32(i as i32 + 1),
                    Value::Text(a.into()),
                    Value::Text(b.into()),
                    Value::Text(k.into()),
                    Value::Text("private-unqualified".into()),
                ]
            })
            .collect(),
            policies: vec![
                None,
                Some(TextCollationId::from_host(1)),
                Some(TextCollationId::from_host(2)),
                Some(TextCollationId::from_host(3)),
                Some(TextCollationId::from_host(999)),
            ],
            scans: Cell::new(0),
            named: parse_query("SELECT Id,A,B FROM T", ParseLimits::default())
                .unwrap()
                .query()
                .clone(),
        }
    }
}
impl Source for Fixture {
    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(&self.columns)
    }
    fn text_collations(&self, _: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        Ok(&self.policies)
    }
    fn query(&self, name: &str) -> Result<Option<&tablecraft_sql::query::Query>, QueryError> {
        Ok((name == "Named").then_some(&self.named))
    }
    fn scan(
        &self,
        _: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        for row in &self.rows {
            visitor(row)?;
        }
        Ok(())
    }
}
pub(super) fn run(
    sql: &str,
    source: &Fixture,
    resolver: Option<&Resolver>,
) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    prepare(parsed.query(), source)?
        .execute(
            source,
            &[],
            ExecutionOptions {
                collation: Some(&DEFAULT),
                collation_resolver: resolver.map(|r| r as &dyn CollationResolver),
                like_mode: Some(tablecraft_expressions::predicates::LikeMode::Ansi89),
                ..Default::default()
            },
        )
        .map(|r| r.rows().to_vec())
}
pub(super) fn ids(values: &[i32]) -> Vec<Vec<Value>> {
    values.iter().map(|v| vec![Value::Int32(*v)]).collect()
}
pub(super) fn texts(values: &[&str]) -> Vec<Vec<Value>> {
    values
        .iter()
        .map(|v| vec![Value::Text((*v).into())])
        .collect()
}
