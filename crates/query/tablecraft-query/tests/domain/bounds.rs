use super::*;
use std::cell::Cell;
use tablecraft_query::QueryLimits;
use tablecraft_sql::query::Query;

struct Host {
    data: MemorySource,
    named: Query,
    scans: Cell<usize>,
    failures: bool,
}
impl Source for Host {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        if self.failures {
            return Err(QueryError::Source);
        }
        Ok((name == "Q").then_some(&self.named))
    }
    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.data.columns(name)
    }
    fn scan(
        &self,
        name: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.data.scan(name, visitor)
    }
}
fn host(sql: &str) -> Host {
    Host {
        data: data(),
        named: parse_query(sql, ParseLimits::default())
            .unwrap()
            .into_query(),
        scans: Cell::new(0),
        failures: false,
    }
}
#[test]
fn named_domains_are_parameterless_and_preparation_does_not_scan_or_resolve_dynamic_names() {
    let source = host("SELECT X FROM Données WHERE X>2 UNION ALL SELECT 8");
    let syntax = parse_query("SELECT DSum('X','Q')", ParseLimits::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    assert_eq!(
        number(
            &plan
                .execute(&source, &[], Default::default())
                .unwrap()
                .rows()[0][0]
        ),
        18.
    );
    assert_eq!(source.scans.get(), 1);
    let source = host("PARAMETERS P LONG; SELECT X+P AS X FROM Données");
    assert_eq!(
        run("SELECT DCount('*','Q')", &source).err(),
        Some(QueryError::Parameter)
    );
    assert_eq!(source.scans.get(), 0);
    let mut source = host("SELECT X FROM Données");
    source.failures = true;
    assert_eq!(
        run("SELECT DCount('*','Q','Missing=1')", &source).err(),
        Some(QueryError::Source)
    );
}
#[test]
fn stored_query_domain_cycles_stop_without_resetting_depth_or_budget() {
    let source = host("SELECT DCount('*','Q') AS X");
    assert_eq!(
        run("SELECT DCount('X','Q')", &source).err(),
        Some(QueryError::StructureLimit)
    );
}
#[test]
fn every_domain_checkpoint_cancels_and_repeated_calls_share_quotas() {
    let source = data();
    let syntax = parse_query("SELECT DSum('X','Données','X>2')", ParseLimits::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let checks = Cell::new(0);
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&|| {
                checks.set(checks.get() + 1);
                false
            }),
            ..Default::default()
        },
    )
    .unwrap();
    for stop in 1..=checks.get() {
        let count = Cell::new(0);
        let error = plan
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    cancelled: Some(&|| {
                        count.set(count.get() + 1);
                        count.get() == stop
                    }),
                    ..Default::default()
                },
            )
            .err();
        assert!(
            matches!(
                error,
                Some(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {stop}: {error:?}"
        );
    }
    let syntax = parse_query(
        "SELECT DCount('*','Données') FROM Données",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    assert!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                limits: QueryLimits {
                    rows: 15,
                    ..Default::default()
                },
                ..Default::default()
            }
        )
        .is_err()
    );
    for limits in [
        QueryLimits {
            bytes: 1000,
            ..Default::default()
        },
        QueryLimits {
            steps: 200,
            ..Default::default()
        },
    ] {
        assert!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    limits,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
}
