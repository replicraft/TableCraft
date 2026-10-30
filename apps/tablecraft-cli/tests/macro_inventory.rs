//! SOURCE-only registration precedes explicit inventory command calls.
#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod base;
#[allow(dead_code)]
#[path = "../../../crates/engines/tablecraft-ace/src/preservation/table_extra/tests/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "guard_preparation/guard_sources.rs"]
mod guard_sources;
#[path = "guard_preparation/inspect.rs"]
mod inspect;
#[path = "macro_inventory/sources.rs"]
mod sources;

use tablecraft_testkit::access as support;
