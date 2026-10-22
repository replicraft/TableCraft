//! Command publication distinguishes operation failures from stdout failures.
use super::Command;
use crate::presentation::Format;
use std::io::{self, Write};

pub(crate) enum ExecutionError {
    Operation(String),
    Output(io::Error),
}
impl Command {
    pub(crate) fn execute(
        self,
        format: Format,
        password: Option<&str>,
        writer: &mut dyn Write,
    ) -> Result<(), ExecutionError> {
        if let Self::Rows(args) = self {
            if args.stream {
                return args.stream_rows(format, password, writer);
            }
            let text = args
                .run(format, password)
                .map_err(|error| ExecutionError::Operation(error.to_string()))?;
            writer
                .write_all(text.as_bytes())
                .map_err(ExecutionError::Output)
        } else {
            let text = self
                .run(format, password)
                .map_err(ExecutionError::Operation)?;
            writer
                .write_all(text.as_bytes())
                .map_err(ExecutionError::Output)
        }
    }
}
