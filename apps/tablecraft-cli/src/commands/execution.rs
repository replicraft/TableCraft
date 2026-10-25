//! Command publication distinguishes operation failures from stdout failures.
use super::{Command, query};
use crate::presentation::Format;
use std::io::{self, Write};

#[derive(Debug)]
pub(crate) enum ExecutionError {
    Operation(String),
    Output(io::Error),
}

impl From<query::Error> for ExecutionError {
    fn from(error: query::Error) -> Self {
        Self::Operation(error.to_string())
    }
}
impl Command {
    pub(crate) fn execute(
        self,
        format: Format,
        password: Option<&str>,
        writer: &mut dyn Write,
    ) -> Result<(), ExecutionError> {
        match self {
            Self::Rows(args) if args.stream => args.stream_rows(format, password, writer),
            Self::Query {
                command: query::Command::Run(args),
            } if args.stream => args.stream_query(format, password, writer),
            command => {
                let text = command
                    .run(format, password)
                    .map_err(ExecutionError::Operation)?;
                writer
                    .write_all(text.as_bytes())
                    .map_err(ExecutionError::Output)
            }
        }
    }
}
