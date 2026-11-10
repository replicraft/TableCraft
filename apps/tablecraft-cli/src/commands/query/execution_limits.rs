//! Explicit query pools remain independent of native reads and output ceilings.
use tablecraft_query::QueryLimits;

#[derive(clap::Args)]
#[group(id = "execution_limits")]
pub(crate) struct Args {
    /// Optional cumulative query allocation budget.
    #[arg(long, value_name = "BYTES", value_parser = clap::value_parser!(u64).range(1..))]
    execution_bytes: Option<u64>,
    /// Optional cumulative query scanned/intermediate/output row budget.
    #[arg(long, value_name = "ROWS", value_parser = clap::value_parser!(u64).range(1..))]
    execution_rows: Option<u64>,
    /// Optional cumulative query comparison/evaluation work budget.
    #[arg(long, value_name = "STEPS", value_parser = clap::value_parser!(u64).range(1..))]
    execution_steps: Option<u64>,
}

impl Args {
    pub(super) fn limits(&self) -> QueryLimits {
        QueryLimits {
            bytes: self.execution_bytes.unwrap_or(QueryLimits::DEFAULT_BYTES),
            rows: self.execution_rows.unwrap_or(QueryLimits::DEFAULT_ROWS),
            steps: self.execution_steps.unwrap_or(QueryLimits::DEFAULT_STEPS),
        }
    }
}
