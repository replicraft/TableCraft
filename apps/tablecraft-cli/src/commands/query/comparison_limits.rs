use clap::Args as ClapArgs;
use tablecraft_core::limits::ResourceBudget;

/// Separate cumulative provider work, shared across policies and equality hints.
#[derive(Clone, Copy, Default, ClapArgs)]
#[group(id = "comparison_limits")]
pub(crate) struct Args {
    /// Per-provider comparison allocation allowance; default 16 MiB, maximum 256 MiB.
    #[arg(long, value_name = "BYTES", requires = "comparison_context", value_parser = clap::value_parser!(u64).range(1..=Args::MAX_BYTES))]
    comparison_bytes: Option<u64>,
    /// Per-provider comparison work allowance; default 1,000,000, maximum 100,000,000.
    #[arg(long, value_name = "STEPS", requires = "comparison_context", value_parser = clap::value_parser!(u64).range(1..=Args::MAX_STEPS))]
    comparison_steps: Option<u64>,
}

impl Args {
    pub(super) const MAX_BYTES: u64 = 256 * 1024 * 1024;
    pub(super) const MAX_STEPS: u64 = 100_000_000;
    const DEFAULT_BYTES: u64 = 16 * 1024 * 1024;
    const DEFAULT_STEPS: u64 = 1_000_000;

    pub(super) fn budget(self) -> ResourceBudget {
        ResourceBudget::new(
            self.comparison_bytes.unwrap_or(Self::DEFAULT_BYTES),
            0,
            self.comparison_steps.unwrap_or(Self::DEFAULT_STEPS),
        )
    }
}
