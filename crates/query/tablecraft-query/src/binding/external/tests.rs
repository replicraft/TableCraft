use crate::{
    QueryError, Source,
    binding::editing::bind_update,
    source::{ExternalSourceRequest, ResolvedExternalSource, SourceKey},
};
use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{parse_query, query::Statement, select::TableSource};

struct Authorizing {
    columns: Vec<Column>,
    lookups: Cell<usize>,
    metadata: Cell<usize>,
}

impl Source for Authorizing {
    fn external_source(
        &self,
        _: ExternalSourceRequest<'_>,
    ) -> Result<Option<ResolvedExternalSource<'_>>, QueryError> {
        self.lookups.set(self.lookups.get() + 1);
        Ok(Some(ResolvedExternalSource::new(SourceKey::new(1), self)))
    }

    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        self.metadata.set(self.metadata.get() + 1);
        Ok(&self.columns)
    }

    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("edit binding must not scan records")
    }
}

#[test]
fn per_table_read_selectors_never_authorize_an_edit_target() {
    let source = Authorizing {
        columns: vec![Column::new("Id".into(), ScalarType::Int32, false).unwrap()],
        lookups: Cell::new(0),
        metadata: Cell::new(0),
    };
    let mut query = parse_query("UPDATE T SET Id=7", Default::default())
        .unwrap()
        .into_query();
    assert!(bind_update(&query, &source).is_ok());
    assert!(source.metadata.get() > 0);
    source.metadata.set(0);

    let Statement::Update(update) = &mut query.statement else {
        unreachable!()
    };
    let TableSource::Table(table) = &mut update.sources[0] else {
        unreachable!()
    };
    table.database = Some("explicitly-authorized-read-source".into());
    assert!(matches!(
        bind_update(&query, &source),
        Err(QueryError::Unsupported)
    ));
    assert_eq!(source.lookups.get(), 0);
    assert_eq!(source.metadata.get(), 0);
}
