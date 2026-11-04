//! Buffered query execution over the retained native source, without provider opens.
use super::{
    Cell, Error, Response,
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
        let mut columns = output.columns(result.columns().len())?;
        for name in result.columns() {
            columns.push(output.text(name)?);
        }
        let mut rows = output.rows(result.rows().len())?;
        for values in result.rows() {
            let mut row = output.row(values.len())?;
            for value in values {
                let cell = match value {
                    ExpressionValue::Value(value) => output.scalar(value, false)?,
                    ExpressionValue::Empty => Cell::expression(output.text("Empty")?),
                    ExpressionValue::Missing => Cell::expression(output.text("Missing")?),
                    ExpressionValue::Error(code) => {
                        Cell::expression(output.text(&format!("#ERROR({code})"))?)
                    }
                    ExpressionValue::StringBytes(_) => return Err(Error::UnpublishedString),
                };
                row.push(cell);
            }
            rows.push(row);
        }
        Ok(Response::Data {
            table: output.text("Query results")?,
            offset: 0,
            columns,
            rows,
            has_more: false,
        })
    }
}
