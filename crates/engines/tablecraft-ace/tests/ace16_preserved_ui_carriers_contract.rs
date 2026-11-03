#![cfg(feature = "ace16-write-preview")]
//! Source-first passive UI carriers and exact Boolean engineering consumers.
#[path = "ace16_preserved_ui_carriers_contract/api.rs"]
mod api;
use tablecraft_testkit::fixtures::access::mutation_fixture as base;
#[path = "ace16_preserved_scalar_updates_contract/catalog_flag.rs"]
mod catalog_flag;
use tablecraft_testkit::fixtures::access::complex_schema as complex_fixture;
#[path = "ace16_preserved_scalar_updates_contract/model.rs"]
mod model;
use tablecraft_testkit::fixtures::access::attachment_modern as modern_attachment;
#[path = "ace16_preserved_scalar_updates_contract/name_map.rs"]
mod name_map;
#[path = "ace16_preserved_ui_carriers_contract/source.rs"]
mod source;

use support as raw;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access as support;

const SCOPE: CatalogScope = CatalogScope::new(1);

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 10_000_000)
}
