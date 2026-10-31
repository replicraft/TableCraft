//! Command publication distinguishes operation failures from stdout failures.
use super::{Command, query};
use crate::{native::publication, presentation::Format};
use std::io::{self, Write};

#[derive(Debug)]
pub(crate) enum ExecutionError {
    Operation(String),
    Output(io::Error),
    EngineeringPublication(publication::Error),
    PublishedCopyOutput(io::Error),
    Journal(query::journal::Error),
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
            Self::Query {
                command: query::Command::PreviewBeforeChange(args),
            } => query::preview_before_change::execute(*args, password, writer),
            Self::Query {
                command: query::Command::FinalizeUpdate(args),
            } => query::journal::finalize(*args, password, writer).map_err(ExecutionError::Journal),
            Self::Query {
                command: query::Command::JournalUpdate(args),
            } => query::journal::update(*args, password, writer).map_err(ExecutionError::Journal),
            Self::Query {
                command: query::Command::RecoverUpdate(args),
            } => query::journal::recover(*args, password, writer).map_err(ExecutionError::Journal),
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
