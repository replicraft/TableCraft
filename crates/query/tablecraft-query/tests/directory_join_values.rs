//! Independent scalar-key join models; no production directory or hash helpers.
use std::cell::{Cell, RefCell};
use tablecraft_core::values::Value;
use tablecraft_db::{ProjectedRow, validate_projection};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, OutputType, QueryError, QueryLimits, Source, prepare,
    source::{ScanConstraint, TextCollationId},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

#[path = "directory_join_values/boundaries.rs"]
mod boundaries;
#[path = "directory_join_values/order.rs"]
mod order;
#[path = "directory_join_values/scale.rs"]
mod scale;

struct Table {
    columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
    policies: Vec<Option<TextCollationId>>,
}

struct Inputs {
    tables: [Table; 2],
    scans: [Cell<usize>; 2],
    visits: [Cell<usize>; 2],
    projections: RefCell<Vec<(usize, Vec<usize>)>>,
    fail_right: bool,
}

impl Inputs {
    fn new(kind: ScalarType, left: &[Option<i128>], right: &[Option<i128>]) -> Self {
        Self {
            tables: [table(kind, left, 11), table(kind, right, 22)],
            scans: [Cell::new(0), Cell::new(0)],
            visits: [Cell::new(0), Cell::new(0)],
            projections: RefCell::new(Vec::new()),
            fail_right: false,
        }
    }

    fn index(name: &str) -> Result<usize, QueryError> {
        match name {
            "L" => Ok(0),
            "R" => Ok(1),
            _ => Err(QueryError::Source),
        }
    }
}

impl Source for Inputs {
    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        Ok(&self.tables[Self::index(name)?].columns)
    }

    fn text_collations(&self, name: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        Ok(&self.tables[Self::index(name)?].policies)
    }

    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("this source supplies explicitly projected rows")
    }

    fn scan_projected(
        &self,
        name: &str,
        indices: &[usize],
        _: Option<ScanConstraint>,
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        let index = Self::index(name)?;
        let table = &self.tables[index];
        validate_projection(indices, table.columns.len())?;
        self.scans[index].set(self.scans[index].get() + 1);
        self.projections
            .borrow_mut()
            .push((index, indices.to_vec()));
        for row in &table.rows {
            self.visits[index].set(self.visits[index].get() + 1);
            visitor(ProjectedRow::select(row, indices)?)?;
        }
        if index == 1 && self.fail_right {
            return Err(QueryError::Source);
        }
        Ok(())
    }
}

fn value(kind: ScalarType, key: Option<i128>) -> Value {
    let Some(key) = key else {
        return Value::Null;
    };

    match kind {
        ScalarType::Boolean => Value::Boolean(key != 0),
        ScalarType::Byte => Value::Byte(u8::try_from(key).unwrap()),
        ScalarType::Int16 => Value::Int16(i16::try_from(key).unwrap()),
        ScalarType::Int32 => Value::Int32(i32::try_from(key).unwrap()),
        ScalarType::Int64 => Value::Int64(i64::try_from(key).unwrap()),
        _ => panic!("independent model uses only exact proven carriers"),
    }
}

fn table(kind: ScalarType, keys: &[Option<i128>], policy: u64) -> Table {
    Table {
        columns: vec![
            Column::new("Unused".into(), ScalarType::Int32, false).unwrap(),
            Column::new("K".into(), kind, true).unwrap(),
            Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
            Column::new("Payload".into(), ScalarType::Text, false).unwrap(),
        ],
        rows: keys
            .iter()
            .enumerate()
            .map(|(index, key)| {
                vec![
                    Value::Text("unselected malformed Int32".into()),
                    value(kind, *key),
                    Value::Int32(i32::try_from(index).unwrap()),
                    Value::Text(format!(" {policy}:🦀:{index} ")),
                ]
            })
            .collect(),
        policies: vec![None, None, None, Some(TextCollationId::from_host(policy))],
    }
}

fn execute(
    sql: &str,
    source: &Inputs,
    options: ExecutionOptions<'_>,
) -> Result<Vec<Vec<Value>>, QueryError> {
    let query = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(query.query(), source)?;
    plan.execute(source, &[], options)
        .map(|result| result.rows().to_vec())
}
