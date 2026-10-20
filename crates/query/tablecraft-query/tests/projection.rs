use std::cell::RefCell;
use tablecraft_core::values::Value;
use tablecraft_query::{
    ExecutionOptions, QueryError, Source, prepare,
    source::{ProjectedRow, ScanConstraint},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};

type Request = (String, Vec<usize>, Option<ScanConstraint>);

struct Projecting {
    columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
    requests: RefCell<Vec<Request>>,
}
impl Projecting {
    fn new() -> Self {
        Self {
            columns: ["Id", "Unused", "GroupKey", "Amount", "Rank", "OuterOnly"]
                .iter()
                .map(|name| Column::new((*name).into(), ScalarType::Int64, false).unwrap())
                .collect(),
            rows: (1..=3)
                .map(|id| {
                    vec![
                        Value::Int64(id),
                        Value::Text("invalid unread payload".into()),
                        Value::Int64(id % 2),
                        Value::Int64(id * 10),
                        Value::Int64(4 - id),
                        Value::Int64(id * 100),
                    ]
                })
                .collect(),
            requests: RefCell::new(vec![]),
        }
    }
}
impl Source for Projecting {
    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(&self.columns)
    }
    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("Projected scan required")
    }
    fn scan_projected(
        &self,
        table: &str,
        indices: &[usize],
        constraint: Option<ScanConstraint>,
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.requests
            .borrow_mut()
            .push((table.into(), indices.to_vec(), constraint));
        if indices.contains(&1) {
            return Err(QueryError::Source);
        }
        for row in &self.rows {
            visitor(ProjectedRow::select(row, indices)?)?;
        }
        Ok(())
    }
}
fn execute(sql: &str, source: &Projecting) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), source)?;
    Ok(plan
        .execute(source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}

#[test]
fn projection_retains_filters_sorting_grouping_and_original_constraint_ordinals() {
    let source = Projecting::new();
    assert_eq!(
        execute("SELECT Amount FROM A WHERE Id>1 ORDER BY Rank", &source).unwrap(),
        [vec![Value::Int64(30)], vec![Value::Int64(20)]]
    );
    assert_eq!(source.requests.borrow()[0].1, [0, 3, 4]);
    let source = Projecting::new();
    assert_eq!(
        execute(
            "SELECT Amount FROM A WHERE Amount>=20 ORDER BY Amount",
            &source
        )
        .unwrap(),
        [vec![Value::Int64(20)], vec![Value::Int64(30)]]
    );
    assert_eq!(source.requests.borrow()[0].1, [3]);
    assert_eq!(source.requests.borrow()[0].2.unwrap().column(), 3);
    let source = Projecting::new();
    assert_eq!(execute("SELECT GroupKey, Sum(Amount) AS Total FROM A GROUP BY GroupKey HAVING Sum(Amount)>0 ORDER BY GroupKey", &source).unwrap(), [vec![Value::Int64(0),Value::Int64(20)],vec![Value::Int64(1),Value::Int64(40)]]);
    assert_eq!(source.requests.borrow()[0].1, [2, 3]);
}

#[test]
fn zero_width_sources_keep_record_cardinality_and_joins_keep_needed_cells() {
    let source = Projecting::new();
    assert_eq!(
        execute("SELECT Count(*) FROM A", &source).unwrap(),
        [vec![Value::Int64(3)]]
    );
    assert!(source.requests.borrow()[0].1.is_empty());
    let source = Projecting::new();
    assert_eq!(
        execute("SELECT Count(*) FROM A AS A, B AS B", &source).unwrap(),
        [vec![Value::Int64(9)]]
    );
    assert!(
        source
            .requests
            .borrow()
            .iter()
            .all(|request| request.1.is_empty())
    );
    let source = Projecting::new();
    assert_eq!(
        execute(
            "SELECT A.Amount, B.Rank FROM A AS A LEFT JOIN B AS B ON A.Id=B.Id+10 ORDER BY A.Id",
            &source
        )
        .unwrap(),
        [
            vec![Value::Int64(10), Value::Null],
            vec![Value::Int64(20), Value::Null],
            vec![Value::Int64(30), Value::Null]
        ]
    );
    assert_eq!(
        source
            .requests
            .borrow()
            .iter()
            .map(|request| request.1.clone())
            .collect::<Vec<_>>(),
        [vec![0, 3], vec![0, 4]]
    );
}

#[test]
fn correlation_and_nested_join_conditions_keep_their_own_scope_maps() {
    let source = Projecting::new();
    assert_eq!(execute("SELECT A.Id, (SELECT Count(*) FROM B AS B WHERE B.Amount>A.OuterOnly) AS N FROM A AS A ORDER BY A.Id", &source).unwrap(), [vec![Value::Int64(1),Value::Int64(0)],vec![Value::Int64(2),Value::Int64(0)],vec![Value::Int64(3),Value::Int64(0)]]);
    assert_eq!(source.requests.borrow()[0].1, [0, 5]);
    assert!(
        source
            .requests
            .borrow()
            .iter()
            .skip(1)
            .all(|request| request.1 == [3])
    );
    let source = Projecting::new();
    assert_eq!(
        execute(
            "SELECT (SELECT (SELECT A.OuterOnly)) AS V FROM A AS A ORDER BY A.Id",
            &source
        )
        .unwrap(),
        [
            vec![Value::Int64(100)],
            vec![Value::Int64(200)],
            vec![Value::Int64(300)]
        ]
    );
    let source = Projecting::new();
    assert_eq!(execute("SELECT A.Amount FROM A AS A INNER JOIN (B AS B INNER JOIN C AS C ON B.Id=C.Id AND EXISTS(SELECT 1 WHERE B.Rank=C.Rank)) ON A.Id=B.Id ORDER BY A.Id", &source).unwrap(), [vec![Value::Int64(10)],vec![Value::Int64(20)],vec![Value::Int64(30)]]);
    assert_eq!(
        source
            .requests
            .borrow()
            .iter()
            .map(|request| request.1.clone())
            .collect::<Vec<_>>(),
        [vec![0, 3], vec![0, 4], vec![0, 4]]
    );
}

#[test]
fn derived_outputs_and_errors_are_retained_while_their_inputs_are_pruned() {
    let source = Projecting::new();
    assert_eq!(
        execute(
            "SELECT D.Id FROM (SELECT Id, Amount+1 AS Plus FROM A) AS D ORDER BY Id",
            &source
        )
        .unwrap(),
        [
            vec![Value::Int64(1)],
            vec![Value::Int64(2)],
            vec![Value::Int64(3)]
        ]
    );
    assert_eq!(source.requests.borrow()[0].1, [0, 3]);
    assert!(
        execute(
            "SELECT D.Id FROM (SELECT Id, 1/0 AS Broken FROM A) AS D",
            &Projecting::new()
        )
        .is_err()
    );
    assert_eq!(
        execute("SELECT Unused FROM A", &Projecting::new()),
        Err(QueryError::Source)
    );
}

#[test]
fn default_projection_preserves_full_row_validation_and_latches_visitor_errors() {
    struct Ignoring {
        columns: Vec<Column>,
        row: Vec<Value>,
    }
    impl Source for Ignoring {
        fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
            Ok(&self.columns)
        }
        fn scan(
            &self,
            _: &str,
            visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
        ) -> Result<(), QueryError> {
            let _ = visitor(&self.row);
            let _ = visitor(&self.row);
            Ok(())
        }
    }
    let mut source = Ignoring {
        columns: vec![Column::new("Id".into(), ScalarType::Int64, false).unwrap()],
        row: vec![Value::Text("invalid carrier".into())],
    };
    let mut visited = 0;
    assert!(
        source
            .scan_projected("A", &[], None, &mut |_| {
                visited += 1;
                Ok(())
            })
            .is_err()
    );
    assert_eq!(visited, 0);
    source.row = vec![Value::Int64(1)];
    assert_eq!(
        source.scan_projected("A", &[0], None, &mut |_| {
            visited += 1;
            Err(QueryError::Cancelled)
        }),
        Err(QueryError::Cancelled)
    );
    assert_eq!(visited, 1);
    for invalid in [&[0, 0][..], &[1][..]] {
        assert!(
            source
                .scan_projected("A", invalid, None, &mut |_| {
                    panic!("invalid projection visited")
                })
                .is_err()
        );
    }
}

#[test]
fn conditional_branches_keep_all_inputs_and_do_not_hide_unselected_source_errors() {
    let source = Projecting::new();
    assert_eq!(
        execute("SELECT IIf(True,Id,Amount) FROM A", &source).unwrap(),
        [
            vec![Value::Int64(1)],
            vec![Value::Int64(2)],
            vec![Value::Int64(3)]
        ]
    );
    assert_eq!(source.requests.borrow()[0].1, [0, 3]);
    assert_eq!(
        execute("SELECT IIf(True,Id,Unused) FROM A", &source),
        Err(QueryError::Source)
    );
}

#[test]
fn nz_keeps_both_projected_operands_including_unused_fallbacks() {
    let source = Projecting::new();
    assert_eq!(
        execute("SELECT Nz(Id,Amount) FROM A", &source).unwrap(),
        [
            vec![Value::Int64(1)],
            vec![Value::Int64(2)],
            vec![Value::Int64(3)]
        ]
    );
    assert_eq!(source.requests.borrow()[0].1, [0, 3]);
    assert_eq!(
        execute("SELECT Nz(Id,Unused) FROM A", &source),
        Err(QueryError::Source)
    );
}

#[test]
fn distinctrow_requires_hidden_selected_record_fields_instead_of_pruning_them() {
    let source = Projecting::new();
    assert_eq!(
        execute("SELECT DISTINCTROW A.Amount FROM A,B", &source),
        Err(QueryError::Source)
    );
    assert_eq!(source.requests.borrow()[0].1, [0, 1, 2, 3, 4, 5]);
    assert!(execute("SELECT DISTINCT A.Amount FROM A,B", &Projecting::new()).is_ok());
}
