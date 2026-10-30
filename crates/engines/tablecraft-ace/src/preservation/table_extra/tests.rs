mod decode;
mod fixture;
mod inspect;
mod program_sources;

use tablecraft_core::limits::ResourceBudget;

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}
