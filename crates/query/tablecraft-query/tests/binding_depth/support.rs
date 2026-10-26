use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::{
    BindingDepth, PreparationOptions, PreparedQuery, QueryError, Source, prepare_with_options,
    source::{CollationResolver, MemorySource, MemoryTable, ProjectedRow, TextCollationId},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{parse_query, query::Query};

const WORD_POLICIES: [Option<TextCollationId>; 1] = [Some(TextCollationId::from_host(7))];

pub const COMPOUND: &str = "SELECT VarType(K) AS T,K AS V,N FROM \
    (SELECT K,Count(*) AS N FROM \
    (SELECT CInt(1) AS Id,IIf(CBool(True),CDec('-0.1'),CCur('-0.1')) AS K \
    UNION ALL SELECT CInt(2) AS Id,IIf(CBool(True),CCur('-0.1'),CDec('-0.1')) AS K \
    UNION ALL SELECT CInt(3) AS Id,IIf(CBool(True),CDbl(-0.1),CDec('-0.1')) AS K) Numbers \
    GROUP BY K) G";

pub struct Data {
    data: MemorySource,
    definitions: Vec<(String, Query)>,
    pub scans: Cell<usize>,
    pub streams: Cell<usize>,
}

impl Data {
    pub fn new(definitions: &[(&str, &str)]) -> Self {
        let numbers = MemoryTable::new(
            "T".into(),
            vec![Column::new("K".into(), ScalarType::Int32, false).unwrap()],
            vec![
                vec![Value::Int32(2)],
                vec![Value::Int32(1)],
                vec![Value::Int32(3)],
            ],
        )
        .unwrap();
        let words = MemoryTable::new(
            "Words".into(),
            vec![Column::new("S".into(), ScalarType::Text, false).unwrap()],
            vec![vec![Value::Text("b".into())], vec![Value::Text("a".into())]],
        )
        .unwrap();

        Self {
            data: MemorySource::new(vec![numbers, words]).unwrap(),
            definitions: definitions
                .iter()
                .map(|(name, text)| ((*name).into(), syntax(text)))
                .collect(),
            scans: Cell::new(0),
            streams: Cell::new(0),
        }
    }
}

impl Source for Data {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(self
            .definitions
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, query)| query))
    }

    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.data.columns(table)
    }

    fn text_collations(&self, table: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        if table.eq_ignore_ascii_case("Words") {
            Ok(&WORD_POLICIES)
        } else {
            Ok(&[])
        }
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
        self.streams.set(self.streams.get() + 1);
        self.data
            .scan_projected_progressive(table, indices, visitor)
    }
}

pub struct Lexical;

impl Collation for Lexical {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

impl CollationResolver for Lexical {
    fn resolve(&self, policy: TextCollationId) -> Result<&dyn Collation, QueryError> {
        if policy == TextCollationId::from_host(7) {
            Ok(self)
        } else {
            Err(QueryError::Source)
        }
    }
}

pub fn syntax(text: &str) -> Query {
    parse_query(text, Default::default()).unwrap().into_query()
}

pub fn extended() -> BindingDepth {
    BindingDepth::new(32).unwrap()
}

pub fn prepared(text: &str, source: &dyn Source) -> PreparedQuery {
    prepare_with_options(
        &syntax(text),
        source,
        PreparationOptions {
            binding_depth: extended(),
            ..Default::default()
        },
    )
    .unwrap()
}

pub fn wrapped(field: &str) -> String {
    let mut expression = field.to_owned();
    for _ in 0..18 {
        expression = format!("CVar({expression})");
    }
    expression
}

pub fn derived() -> String {
    let mut text = "SELECT K FROM T".to_owned();
    for _ in 0..5 {
        text = format!("SELECT K FROM ({text}) D");
    }
    text
}
