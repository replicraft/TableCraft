mod decode;
mod fixture;
#[cfg(feature = "ace16-write-preview")]
mod guard_selector_sources;
#[cfg(feature = "ace16-write-preview")]
mod guard_sources;
mod inspect;
mod program_sources;

use tablecraft_core::limits::ResourceBudget;

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}
