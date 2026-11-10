use clap::Args as ClapArgs;
use tablecraft_core::limits::ResourceBudget;

/// Separate cumulative provider work, shared across policies and equality hints.
#[derive(Clone, Copy, Default, ClapArgs)]
#[group(id = "comparison_limits")]
pub(crate) struct Args {
    /// Optional cumulative per-provider comparison allocation budget.
    #[arg(long, value_name = "BYTES", requires = "comparison_context", value_parser = clap::value_parser!(u64).range(1..))]
    comparison_bytes: Option<u64>,
    /// Optional cumulative per-provider comparison work budget.
    #[arg(long, value_name = "STEPS", requires = "comparison_context", value_parser = clap::value_parser!(u64).range(1..))]
    comparison_steps: Option<u64>,
}

impl Args {
    pub(super) fn budget(self) -> ResourceBudget {
        ResourceBudget::new(
            self.comparison_bytes.unwrap_or(u64::MAX),
            0,
            self.comparison_steps.unwrap_or(u64::MAX),
        )
    }
}
