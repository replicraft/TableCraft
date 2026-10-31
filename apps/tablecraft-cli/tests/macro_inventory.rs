//! SOURCE-only registration precedes explicit inventory command calls.
use tablecraft_testkit::access::mutation_fixture as base;
#[allow(dead_code)]
#[path = "guard_preparation/guard_sources.rs"]
mod guard_sources;
#[path = "guard_preparation/inspect.rs"]
mod inspect;
#[path = "macro_inventory/sources.rs"]
mod sources;

use tablecraft_testkit::access as support;
