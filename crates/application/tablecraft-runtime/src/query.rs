//! Buffered query execution over the retained native source, without provider opens.
use super::{
    Error, Response,
    controls::{SCOPE, budget, check},
    display::Presentation,
    document::Document,
};
use tablecraft_access::NativeSource;
use tablecraft_core::values::ExpressionValue;
use tablecraft_query::{ExecutionOptions, QueryLimits, execute_with_schema_discovery};
use tablecraft_sql::{ParseLimits, parse_query};

const SQL_BYTES: usize = 64 * 1024;

impl Document {
    pub(super) fn query(
        &mut self,
        sql: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        check(cancelled)?;
        if sql.len() > SQL_BYTES {
            return Err(Error::SqlLimit);
        }
        let parsed = parse_query(sql, ParseLimits::default())?;
        let source = NativeSource::new(&mut self.source, SCOPE, budget(), cancelled)?;
        let result = execute_with_schema_discovery(
            parsed.query(),
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(cancelled),
                limits: QueryLimits {
                    bytes: 16 * 1024 * 1024,
                    rows: 10_000,
                    steps: 1_000_000,
                },
                ..Default::default()
            },
        )?;
        let mut output = Presentation::new(cancelled);
        let columns = result
            .columns()
            .iter()
            .map(|name| output.text(name))
            .collect::<Result<_, _>>()?;
        let rows = result
            .rows()
            .iter()
            .map(|row| {
                row.iter()
                    .map(|value| match value {
                        ExpressionValue::Value(value) => output.value(value),
                        ExpressionValue::Empty => output.text("Empty"),
                        ExpressionValue::Missing => output.text("Missing"),
                        ExpressionValue::Error(code) => output.text(&format!("#ERROR({code})")),
                        ExpressionValue::StringBytes(_) => Err(Error::UnpublishedString),
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<_, _>>()?;
        Ok(Response::Data {
            table: "Query results".to_owned(),
            offset: 0,
            columns,
            rows,
            has_more: false,
        })
    }
}
