//! Explicit per-value and final-output allowances for query retrieval.
use crate::output::MAX_LARGE_OUTPUT_BYTES;
use clap::Args as ClapArgs;
use tablecraft_expressions::evaluation::EvalLimits;

#[derive(ClapArgs)]
pub(crate) struct Args {
    /// Individual query Text bytes; default 65,536, maximum 16,777,216.
    #[arg(long, value_name = "BYTES", value_parser = clap::value_parser!(u64).range(1..=EvalLimits::MAX_TEXT_BYTES as u64))]
    pub(crate) text_bytes: Option<u64>,
    /// Buffered display and final transport bytes; default 1 MiB, maximum 16 MiB.
    #[arg(long, value_name = "BYTES", value_parser = clap::value_parser!(u64).range(1..=MAX_LARGE_OUTPUT_BYTES as u64))]
    pub(crate) output_bytes: Option<u64>,
}
