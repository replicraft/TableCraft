//! Typed list reads retain existing structural/named limits on a bounded stack.
use std::{
    cell::Cell,
    fs::{self, File},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_query::{
    BindingDepth, CollectionQueryResult, PreparationOptions, QueryError, QueryValue, Source,
    prepare, prepare_with_options,
    source::{MultivalueList, MultivalueVisitor, ProjectedRow},
};
use tablecraft_schema::{Column, ScalarType, SchemaName};
use tablecraft_sql::{
    parse_query,
    query::{Query, ReadBody, Statement},
    select::TableSource,
};

const CHILD_ENV: &str = "TABLECRAFT_COLLECTION_DEPTH_CHILD";
const CHILD_TIMEOUT: Duration = Duration::from_secs(30);
const WORKER_STACK_BYTES: usize = 1024 * 1024;

struct Data {
    columns: Vec<Column>,
    unavailable: Vec<SchemaName>,
    elements: Vec<Column>,
    rows: Vec<Vec<Value>>,
    lists: Vec<Vec<Value>>,
    definitions: Vec<(String, Query)>,
    collection_scans: Cell<usize>,
    scalar_scans: Cell<usize>,
}

impl Data {
    fn chain(count: usize) -> Self {
        let definitions = (0..count)
            .map(|index| {
                let target = if index + 1 == count {
                    "Data".to_owned()
                } else {
                    format!("Q{}", index + 1)
                };
                (
                    format!("Q{index}"),
                    syntax(&format!("SELECT Id,Tags FROM {target}")),
                )
            })
            .collect();
        Self {
            columns: vec![Column::new("Id".into(), ScalarType::Int32, false).unwrap()],
            unavailable: vec![SchemaName::new("Tags".into()).unwrap()],
            elements: vec![Column::new("Tags".into(), ScalarType::Int32, true).unwrap()],
            rows: vec![vec![Value::Int32(1)], vec![Value::Int32(2)]],
            lists: vec![
                vec![Value::Null, Value::Int32(100), Value::Int32(100)],
                vec![],
            ],
            definitions,
            collection_scans: Cell::new(0),
            scalar_scans: Cell::new(0),
        }
    }

    fn no_scans(&self) {
        assert_eq!(self.scalar_scans.get(), 0);
        assert_eq!(self.collection_scans.get(), 0);
    }
}

impl Source for Data {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(self
            .definitions
            .iter()
            .find_map(|(label, query)| label.eq_ignore_ascii_case(name).then_some(query)))
    }

    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        if !table.eq_ignore_ascii_case("Data") {
            return Err(QueryError::UnknownName);
        }
        Ok(&self.columns)
    }

    fn unavailable_columns(&self, _: &str) -> Result<&[SchemaName], QueryError> {
        Ok(&self.unavailable)
    }

    fn multivalue_columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(&self.elements)
    }

    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scalar_scans.set(self.scalar_scans.get() + 1);
        Err(QueryError::Source)
    }

    fn scan_multivalues(
        &self,
        table: &str,
        indices: &[usize],
        collections: &[usize],
        visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        assert!(table.eq_ignore_ascii_case("Data"));
        assert_eq!(collections, [0]);
        self.collection_scans.set(self.collection_scans.get() + 1);
        for (row, list) in self.rows.iter().zip(&self.lists) {
            let list: &dyn MultivalueList = list;
            visitor(ProjectedRow::select(row, indices)?, &[list])?;
        }
        Ok(())
    }
}

fn syntax(sql: &str) -> Query {
    parse_query(sql, Default::default()).unwrap().into_query()
}

fn assert_complete(result: CollectionQueryResult, source: &Data) {
    assert_eq!(result.columns(), ["Id", "Tags"]);
    assert_eq!(result.rows().len(), 2);
    for (position, row) in result.rows().iter().enumerate() {
        assert_eq!(row.len(), 2);
        assert_eq!(
            row[0],
            QueryValue::Scalar(ExpressionValue::Value(source.rows[position][0].clone()))
        );
        let QueryValue::Multivalue(list) = &row[1] else {
            panic!("the typed list must not become a scalar or disappear");
        };
        assert_eq!(list.values(), source.lists[position]);
    }
    assert_eq!(source.collection_scans.get(), 1);
    assert_eq!(source.scalar_scans.get(), 0);
}

fn named_limit() {
    let source = Data::chain(50);
    let plan = prepare(&syntax("SELECT Id,Tags FROM Q0"), &source).unwrap();
    source.no_scans();
    let result = plan
        .execute_collections(&source, &[], Default::default())
        .unwrap();
    assert_complete(result, &source);

    let source = Data::chain(51);
    assert_eq!(
        prepare(&syntax("SELECT Id,Tags FROM Q0"), &source).err(),
        Some(QueryError::StructureLimit)
    );
    source.no_scans();
}

fn joined_named_limits() {
    for count in [8, 50] {
        let mut source = Data::chain(count);
        for (index, (_, query)) in source.definitions.iter_mut().enumerate() {
            let target = if index + 1 == count {
                "Data".to_owned()
            } else {
                format!("Q{}", index + 1)
            };
            *query = syntax(&format!(
                "SELECT D.Id,D.Tags FROM {target} AS D \
                 LEFT JOIN (SELECT 1 AS MatchKey) AS C ON 1=1"
            ));
        }

        let prepared = prepare(&syntax("SELECT Id,Tags FROM Q0"), &source);
        source.no_scans();
        if count == 8 {
            let result = prepared
                .unwrap()
                .execute_collections(&source, &[], Default::default())
                .unwrap();
            assert_complete(result, &source);
        } else {
            // Named definitions and relation nodes share the engineering bound.
            // This compound shape does not claim fifty-level support merely
            // because fifty plain named SELECTs fit the independent name limit.
            let error = match prepared {
                Ok(plan) => plan
                    .execute_collections(&source, &[], Default::default())
                    .err(),
                Err(error) => Some(error),
            };
            assert_eq!(error, Some(QueryError::StructureLimit));
            source.no_scans();
        }
    }
}

fn derived_limits() {
    let source = Data::chain(0);
    let mut sql = "SELECT Id,Tags FROM Data".to_owned();
    for _ in 0..5 {
        sql = format!("SELECT Id,Tags FROM ({sql}) D");
    }
    let query = syntax(&sql);
    assert_eq!(
        prepare(&query, &source).err(),
        Some(QueryError::StructureLimit)
    );
    source.no_scans();

    let plan = prepare_with_options(
        &query,
        &source,
        PreparationOptions {
            binding_depth: BindingDepth::new(32).unwrap(),
            ..Default::default()
        },
    )
    .unwrap();
    source.no_scans();
    // Execution uses the captured plan depth; default execution options do not
    // silently rebind or enlarge the prepared structural allowance.
    let result = plan
        .execute_collections(&source, &[], Default::default())
        .unwrap();
    assert_complete(result, &source);
}

fn absolute_limit() {
    let source = Data::chain(0);
    let mut query = syntax("SELECT Id,Tags FROM Data");
    for _ in 0..40 {
        let Statement::Read(inner) = query.statement else {
            unreachable!()
        };
        let mut outer = syntax("SELECT Id,Tags FROM (SELECT Id,Tags FROM Data) D");
        let Statement::Read(read) = &mut outer.statement else {
            unreachable!()
        };
        let ReadBody::Select(select) = &mut read.body else {
            unreachable!()
        };
        let TableSource::Derived { query: target, .. } = &mut select.from[0] else {
            unreachable!()
        };
        *target = inner;
        query = outer;
    }
    assert_eq!(
        prepare_with_options(
            &query,
            &source,
            PreparationOptions {
                binding_depth: BindingDepth::new(32).unwrap(),
                ..Default::default()
            }
        )
        .err(),
        Some(QueryError::StructureLimit)
    );
    source.no_scans();
}

fn isolated_worker(test_name: &str, run: fn()) {
    if std::env::var_os(CHILD_ENV).is_none() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "tablecraft-collection-depth-{}-{nonce}.log",
            std::process::id()
        ));
        let diagnostics = File::options()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test_name, "--test-threads=1", "--nocapture"])
            .env(CHILD_ENV, "1")
            .stdout(Stdio::null())
            .stderr(Stdio::from(diagnostics))
            .spawn()
            .unwrap();
        let started = Instant::now();
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                let _ = child.wait_with_output().unwrap();
                let diagnostics = fs::read_to_string(&path).unwrap();
                assert!(
                    status.success(),
                    "{test_name}: {diagnostics}; preserved {}",
                    path.display()
                );
                fs::remove_file(&path).unwrap();
                return;
            }
            if started.elapsed() >= CHILD_TIMEOUT {
                let _ = child.kill();
                let diagnostics = fs::read_to_string(&path).unwrap_or_default();
                panic!(
                    "{test_name} timed out: {diagnostics}; preserved {}",
                    path.display()
                );
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    std::thread::Builder::new()
        .stack_size(WORKER_STACK_BYTES)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn typed_lists_cross_fifty_named_sources_but_the_fifty_first_is_refused() {
    isolated_worker(
        "typed_lists_cross_fifty_named_sources_but_the_fifty_first_is_refused",
        named_limit,
    );
}

#[test]
fn typed_derived_lists_use_the_existing_selected_binding_depth() {
    isolated_worker(
        "typed_derived_lists_use_the_existing_selected_binding_depth",
        derived_limits,
    );
}

#[test]
fn typed_derived_lists_keep_the_absolute_structural_guard() {
    isolated_worker(
        "typed_derived_lists_keep_the_absolute_structural_guard",
        absolute_limit,
    );
}

#[test]
fn typed_joined_named_sources_share_a_checked_combined_depth() {
    isolated_worker(
        "typed_joined_named_sources_share_a_checked_combined_depth",
        joined_named_limits,
    );
}
