//! Inert file-receive route for explicit desktop and terminal launches.
use super::{error::Error, inspect};
use crate::presentation::Format;
use std::path::PathBuf;

#[derive(clap::Args)]
pub(crate) struct Args {
    /// Explicit local file path; its contents are inspected, never executed.
    #[arg(value_name = "PATH")]
    path: PathBuf,
    /// Wait for Enter after publication only when stdin and stdout are terminals.
    #[arg(long)]
    pub(super) wait: bool,
}

impl Args {
    pub(super) fn run(self, format: Format, password: Option<&str>) -> Result<String, Error> {
        inspect::run(
            &self.path,
            inspect::Mode::Header,
            format,
            password,
            None,
            None,
        )
    }
}
