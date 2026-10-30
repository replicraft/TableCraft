//! Source-first passive UI carriers and exact Boolean engineering consumers.
#[path = "ace16_preserved_ui_carriers_contract/api.rs"]
mod api;
#[path = "mutation/fixture.rs"]
pub mod base;
#[path = "ace16_preserved_scalar_updates_contract/catalog_flag.rs"]
mod catalog_flag;
#[path = "complex_schema/fixture.rs"]
pub mod complex_fixture;
#[path = "ace16_preserved_scalar_updates_contract/model.rs"]
mod model;
#[path = "attachment_modern/fixture.rs"]
pub mod modern_attachment;
#[path = "ace16_preserved_scalar_updates_contract/name_map.rs"]
mod name_map;
#[path = "attachments/fixture.rs"]
pub mod old_attachment;
#[path = "ace16_preserved_ui_carriers_contract/source.rs"]
mod source;

use support as raw;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

const SCOPE: CatalogScope = CatalogScope::new(1);

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 10_000_000)
}
