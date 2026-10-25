use tablecraft_core::values::Value;
use tablecraft_expressions::functions::conversions::text::NumberFormat;
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryResult, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{parse_query, query::Query};

pub const PERMUTATIONS: [[i32; 3]; 6] = [
    [1, 2, 3],
    [1, 3, 2],
    [2, 1, 3],
    [2, 3, 1],
    [3, 1, 2],
    [3, 2, 1],
];

pub struct Numbers {
    data: MemorySource,
    definition: Query,
}

impl Numbers {
    pub fn new(order: [i32; 3], single: &str, low: &str, high: &str) -> Self {
        let data = MemorySource::new(vec![
            MemoryTable::new(
                "Inputs".into(),
                vec![Column::new("Id".into(), ScalarType::Int32, false).unwrap()],
                order.into_iter().map(|id| vec![Value::Int32(id)]).collect(),
            )
            .unwrap(),
        ])
        .unwrap();
        let sql = format!(
            "SELECT Id,IIf(Id=2,CSng({single}),IIf(Id=1,CDbl({low}),CDbl({high}))) AS K FROM Inputs"
        );
        let definition = parse_query(&sql, Default::default())
            .unwrap()
            .query()
            .clone();

        Self { data, definition }
    }

    pub fn normal(order: [i32; 3]) -> Self {
        Self::new(order, "1", "1", "1.00000001")
    }
}

impl Source for Numbers {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(name
            .eq_ignore_ascii_case("Numbers")
            .then_some(&self.definition))
    }

    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.data.columns(table)
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.data.scan(table, visitor)
    }
}

pub fn run(sql: &str, source: &dyn Source, declared: bool) -> Result<QueryResult, QueryError> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), source)?;
    plan.execute(source, &[], options(declared))
}

pub fn options(declared: bool) -> ExecutionOptions<'static> {
    ExecutionOptions {
        declared_integral: declared,
        number_format: Some(NumberFormat::Invariant),
        ..Default::default()
    }
}

pub fn number(value: &Value) -> f64 {
    match value {
        Value::Single(value) => f64::from(value.to_float()),
        Value::Double(value) => value.to_float(),
        other => panic!("unexpected floating carrier {other:?}"),
    }
}
