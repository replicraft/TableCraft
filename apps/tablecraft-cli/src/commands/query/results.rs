//! Buffered query presentation; collection values retain their typed boundary.
use super::{Error, output::TableOutput};
use tablecraft_query::{CollectionQueryResult, ExpressionQueryResult, QueryResult, QueryValue};

pub(super) enum Results {
    Expressions(ExpressionQueryResult),
    Collections(CollectionQueryResult),
}

impl Results {
    pub(super) fn render(&self, limit: usize) -> Result<String, Error> {
        match self {
            Self::Expressions(result) => render(result, limit, |output, value| {
                output.expression_value(value).map_err(Error::Output)
            }),
            Self::Collections(result) => render(result, limit, |output, value| match value {
                QueryValue::Scalar(value) => output.expression_value(value).map_err(Error::Output),
                QueryValue::AbsentMultivalue => output.display(&"NULL").map_err(Error::Output),
                QueryValue::Multivalue(values) => {
                    output.display(&"[").map_err(Error::Output)?;
                    for (index, value) in values.values().iter().enumerate() {
                        if index != 0 {
                            output.display(&", ").map_err(Error::Output)?;
                        }
                        output.value(value).map_err(Error::Output)?;
                    }
                    output.display(&"]").map_err(Error::Output)
                }
            }),
        }
    }
}

fn render<T>(
    result: &QueryResult<T>,
    limit: usize,
    mut cell: impl FnMut(&mut TableOutput, &T) -> Result<(), Error>,
) -> Result<String, Error> {
    let mut output = TableOutput::with_limit(limit);
    for (index, name) in result.columns().iter().enumerate() {
        if index != 0 {
            output.separator().map_err(Error::Output)?;
        }
        output
            .header(Some(name), index + 1)
            .map_err(Error::Output)?;
    }
    output.newline().map_err(Error::Output)?;

    for row in result.rows() {
        for (index, value) in row.iter().enumerate() {
            if index != 0 {
                output.separator().map_err(Error::Output)?;
            }
            cell(&mut output, value)?;
        }
        output.newline().map_err(Error::Output)?;
    }
    Ok(output.finish())
}
