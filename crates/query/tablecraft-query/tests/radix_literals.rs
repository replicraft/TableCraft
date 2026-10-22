use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_query::{
    ExecutionOptions, QueryError, Source, execute_with_schema_discovery, prepare,
    source::MemorySource,
};
use tablecraft_schema::Column;
use tablecraft_sql::{parse_query, query::Query};

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::new(Vec::new()).unwrap();
    let syntax = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(syntax.query(), &source)?
        .execute(&source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}

#[test]
fn signed_radix_values_keep_types_through_evaluation_and_read_consumers() {
    assert_eq!(
        run(
            "SELECT &HFF,&HFFFF,&H10000,&HFFFFFFFF,&O177777,-&H8000,VarType(&HFF),VarType(&H10000)"
        )
        .unwrap(),
        [vec![
            Value::Int16(255),
            Value::Int16(-1),
            Value::Int32(65536),
            Value::Int32(-1),
            Value::Int16(-1),
            Value::Int32(32768),
            Value::Int16(2),
            Value::Int16(3)
        ]]
    );
    assert_eq!(run("SELECT Sum(X),VarType(First(X)) FROM (SELECT &H1 AS X UNION ALL SELECT &O2) AS Q WHERE X<&H3").unwrap(), [vec![Value::Int64(3), Value::Int16(2)]]);
    assert_eq!(
        run("SELECT Eval('&HFFFF'),(&H1+&O2)*2,(SELECT &H80000000)").unwrap(),
        [vec![
            Value::Int16(-1),
            Value::Int64(6),
            Value::Int32(i32::MIN)
        ]]
    );
    assert!(run("SELECT 1,&H100000000").is_err());
}

struct Views {
    query: Query,
    scans: Cell<usize>,
}
impl Source for Views {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(name.eq_ignore_ascii_case("Piv").then_some(&self.query))
    }
    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Err(QueryError::UnknownName)
    }
    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        Err(QueryError::Source)
    }
}

#[test]
fn fixed_radix_headings_bind_without_scans_and_keep_pivot_carriers() {
    let syntax = parse_query("TRANSFORM Sum(X) SELECT G FROM (SELECT 1 AS G,&H2 AS P,7 AS X) AS Q GROUP BY G PIVOT P IN (&O2)", Default::default()).unwrap();
    let source = Views {
        query: syntax.into_query(),
        scans: Cell::new(0),
    };
    let syntax = parse_query("SELECT G,[2] FROM Piv", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    let result = plan
        .execute(&source, &[], ExecutionOptions::default())
        .unwrap();
    assert_eq!(result.rows(), [vec![Value::Int64(1), Value::Int64(7)]]);
    assert_eq!(source.scans.get(), 0);
    assert!(
        execute_with_schema_discovery(syntax.query(), &source, &[], Default::default()).is_ok()
    );
    assert_eq!(source.scans.get(), 0);
}

#[test]
fn equivalent_signed_headings_are_not_distinct_metadata_columns() {
    let query = parse_query("TRANSFORM Sum(X) SELECT G FROM (SELECT 1 AS G,1 AS X) AS Q GROUP BY G PIVOT X IN (&HFFFF,-1)", Default::default()).unwrap().into_query();
    let source = Views {
        query,
        scans: Cell::new(0),
    };
    let syntax = parse_query("SELECT * FROM Piv", Default::default()).unwrap();
    assert_eq!(
        prepare(syntax.query(), &source).err(),
        Some(QueryError::AmbiguousName)
    );
    assert_eq!(source.scans.get(), 0);
}
