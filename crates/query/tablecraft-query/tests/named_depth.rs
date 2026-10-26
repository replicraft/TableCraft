//! Named dependency limits are independent of each definition's expression depth.
use std::{
    cell::Cell,
    fs::{self, File},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, Source, execute_with_schema_discovery, prepare,
    source::{MemorySource, MemoryTable, ProjectedRow},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{parse_query, query::Query};

#[path = "named_depth/shapes.rs"]
mod shapes;

const STACK_CHILD_ENV: &str = "TABLECRAFT_NAMED_STACK_PROBE_CHILD";
const STACK_PROBE_TIMEOUT: Duration = Duration::from_secs(30);

struct Definitions {
    data: MemorySource,
    queries: Vec<(String, Query)>,
    scans: Cell<usize>,
    diagnostic_lookups: Cell<usize>,
    diagnostics: bool,
}

impl Definitions {
    fn chain(count: usize) -> Self {
        let queries = (0..count)
            .map(|index| {
                let target = if index + 1 == count {
                    "Data".to_owned()
                } else {
                    format!("Q{}", index + 1)
                };
                (
                    format!("Q{index}"),
                    syntax(&format!("SELECT ID FROM {target}")),
                )
            })
            .collect();
        Self {
            data: MemorySource::new(vec![
                MemoryTable::new(
                    "Data".into(),
                    vec![Column::new("ID".into(), ScalarType::Int32, false).unwrap()],
                    vec![vec![Value::Int32(1)], vec![Value::Int32(2)]],
                )
                .unwrap(),
            ])
            .unwrap(),
            queries,
            scans: Cell::new(0),
            diagnostic_lookups: Cell::new(0),
            diagnostics: false,
        }
    }
}

impl Source for Definitions {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        if self.diagnostics {
            let lookups = self.diagnostic_lookups.get() + 1;
            self.diagnostic_lookups.set(lookups);
            if lookups.is_multiple_of(25) {
                eprintln!(
                    "domain probe: metadata lookups {lookups}, scans {}",
                    self.scans.get()
                );
            }
        }
        Ok(self
            .queries
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, query)| query))
    }

    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.data.columns(table)
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.data.scan(table, visitor)
    }

    fn scan_projected_progressive(
        &self,
        table: &str,
        indices: &[usize],
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.data
            .scan_projected_progressive(table, indices, visitor)
    }
}

fn syntax(sql: &str) -> Query {
    parse_query(sql, Default::default()).unwrap().into_query()
}

fn run_fifty() {
    let source = Definitions::chain(50);
    let plan = prepare(&syntax("SELECT ID FROM Q0"), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    let result = plan.execute(&source, &[], Default::default()).unwrap();
    assert_eq!(
        result.rows(),
        [vec![Value::Int32(1)], vec![Value::Int32(2)]]
    );
    assert_eq!(source.scans.get(), 1);
}

#[test]
fn fifty_definitions_prepare_without_scanning_and_execute_once_at_the_leaf() {
    run_fifty();
}

#[test]
fn a_fifty_first_definition_and_a_deep_cycle_fail_before_scans() {
    let source = Definitions::chain(51);
    assert_eq!(
        prepare(&syntax("SELECT ID FROM Q0"), &source).err(),
        Some(QueryError::StructureLimit)
    );
    assert_eq!(source.scans.get(), 0);

    let mut cycle = Definitions::chain(50);
    cycle.queries[49].1 = syntax("SELECT ID FROM Q0");
    assert_eq!(
        prepare(&syntax("SELECT ID FROM Q0"), &cycle).err(),
        Some(QueryError::DependencyCycle)
    );
    assert_eq!(cycle.scans.get(), 0);
}

#[test]
fn staged_and_progressive_execution_keep_the_same_fifty_definition_scope() {
    let source = Definitions::chain(50);
    let query = syntax("SELECT ID FROM Q0");
    let result = execute_with_schema_discovery(&query, &source, &[], Default::default()).unwrap();
    assert_eq!(result.rows().len(), 2);
    assert_eq!(source.scans.get(), 1);

    let source = Definitions::chain(50);
    let plan = prepare(&query, &source).unwrap();
    let mut rows = Vec::new();
    let summary = plan
        .execute_progressive(&source, &[], Default::default(), &mut |row| {
            rows.push(row.to_vec());
            Ok::<(), ()>(())
        })
        .unwrap();
    assert_eq!(rows, [vec![Value::Int32(1)], vec![Value::Int32(2)]]);
    assert_eq!(summary.scanned_rows, 2);
    assert_eq!(source.scans.get(), 1);
}

fn isolated_worker(test_name: &str, run: fn()) {
    if std::env::var_os(STACK_CHILD_ENV).is_none() {
        let identity = test_name.replace("::", "-");
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let diagnostics_path = std::env::temp_dir().join(format!(
            "tablecraft-named-stack-{}-{nonce}-{identity}.log",
            std::process::id(),
        ));
        let diagnostics_file = File::options()
            .write(true)
            .create_new(true)
            .open(&diagnostics_path)
            .unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test_name, "--test-threads=1", "--nocapture"])
            .env(STACK_CHILD_ENV, "1")
            .stdout(Stdio::null())
            .stderr(Stdio::from(diagnostics_file))
            .spawn()
            .unwrap();
        let started = Instant::now();
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                let _ = child.wait_with_output().unwrap();
                let diagnostics = fs::read_to_string(&diagnostics_path).unwrap();
                assert!(
                    status.success(),
                    "{test_name}: {diagnostics}; preserved {}",
                    diagnostics_path.display()
                );
                // With --nocapture, successful checked refusals are visible as
                // refusals rather than being presented as full shape support.
                eprint!("{diagnostics}");
                fs::remove_file(&diagnostics_path).unwrap();
                break;
            }
            if started.elapsed() >= STACK_PROBE_TIMEOUT {
                let _ = child.kill();
                let diagnostics = fs::read_to_string(&diagnostics_path).unwrap_or_default();
                panic!(
                    "isolated named-query worker stack probe timed out: {test_name}; {diagnostics}; preserved {}",
                    diagnostics_path.display(),
                );
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        return;
    }
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn fifty_named_sources_fit_a_one_mib_worker_stack_in_an_isolated_process() {
    isolated_worker(
        "fifty_named_sources_fit_a_one_mib_worker_stack_in_an_isolated_process",
        run_fifty,
    );
}

#[test]
fn staged_preparation_still_obeys_the_shared_work_limit() {
    let source = Definitions::chain(50);
    let options = ExecutionOptions {
        limits: QueryLimits {
            steps: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        execute_with_schema_discovery(&syntax("SELECT ID FROM Q0"), &source, &[], options),
        Err(QueryError::Limit(_))
    ));
    assert_eq!(source.scans.get(), 0);
}
