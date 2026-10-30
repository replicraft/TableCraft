//! SOURCE-only rich-table projection gate before frontend preservation APIs.
use tablecraft_testkit::access::mutation_fixture as base;
#[path = "../../../engines/tablecraft-ace/tests/ace16_preserved_scalar_updates_contract/catalog_flag.rs"]
mod catalog_flag;
#[path = "../../../engines/tablecraft-ace/tests/complex_schema/fixture.rs"]
pub mod complex_fixture;
#[path = "../../../engines/tablecraft-ace/tests/ace16_preserved_scalar_updates_contract/frontend_source.rs"]
mod frontend_source;
#[path = "../../../engines/tablecraft-ace/tests/ace16_preserved_scalar_updates_contract/model.rs"]
mod model;
#[path = "../../../engines/tablecraft-ace/tests/attachment_modern/fixture.rs"]
pub mod modern_attachment;
#[path = "../../../engines/tablecraft-ace/tests/attachments/fixture.rs"]
pub mod old_attachment;

use support as raw;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

const SCOPE: CatalogScope = CatalogScope::new(1);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}
