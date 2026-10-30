//! Engineering consumers of the independently qualified rich-table source.
#[path = "preserved_boolean_updates/api.rs"]
mod api;
use tablecraft_testkit::access::mutation_fixture as base;
#[path = "../../../crates/engines/tablecraft-ace/tests/ace16_preserved_scalar_updates_contract/catalog_flag.rs"]
mod catalog_flag;
#[path = "../../../crates/engines/tablecraft-ace/tests/complex_schema/fixture.rs"]
pub mod complex_fixture;
#[path = "../../../crates/engines/tablecraft-ace/tests/ace16_preserved_scalar_updates_contract/frontend_source.rs"]
mod frontend_source;
#[path = "../../../crates/engines/tablecraft-ace/tests/ace16_preserved_scalar_updates_contract/model.rs"]
mod model;
#[path = "../../../crates/engines/tablecraft-ace/tests/attachment_modern/fixture.rs"]
pub mod modern_attachment;
#[path = "../../../crates/engines/tablecraft-ace/tests/attachments/fixture.rs"]
pub mod old_attachment;

use support as raw;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

const SCOPE: CatalogScope = CatalogScope::new(1);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}
