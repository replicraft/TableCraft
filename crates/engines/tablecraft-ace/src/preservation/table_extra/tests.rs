mod decode;
#[cfg(feature = "ace16-write-preview")]
pub(crate) mod guard_selector_sources;
#[cfg(feature = "ace16-write-preview")]
pub(crate) mod guard_sources;
mod inspect;
#[cfg(feature = "ace16-write-preview")]
pub(crate) mod multievent_sources;
#[cfg(feature = "ace16-write-preview")]
mod observed_macro_inventory;
mod program_sources;
#[cfg(feature = "ace16-write-preview")]
mod setfield_sources;

use tablecraft_core::limits::ResourceBudget;

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}
