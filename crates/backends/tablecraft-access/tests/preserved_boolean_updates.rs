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

use int32_sources::model as int32_model;
use model as rich;
use support as raw;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

const SCOPE: CatalogScope = CatalogScope::new(1);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

#[path = "preserved_boolean_updates/int32_api.rs"]
mod int32_api;
#[path = "../../../engines/tablecraft-ace/tests/ace16_preserved_scalar_updates_contract/int32_sources.rs"]
mod int32_sources;

// Reuse the actual SOURCE model; framing-only members belong to its native qualifiers.
#[path = "preserved_boolean_updates/short_text_api.rs"]
mod short_text_api;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/ace16_preserved_scalar_updates_contract/short_text_sources/model.rs"]
mod short_text_model;

#[path = "preserved_boolean_updates/growth_api.rs"]
mod growth_api;
// Source-only geometry helpers are exercised by the actual engine qualifiers.
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/ace16_preserved_scalar_updates_contract/growing_sources/model.rs"]
mod growth_model;

#[path = "preserved_boolean_updates/scalar_api.rs"]
mod scalar_api;
// The actual shared fixture also owns SOURCE-only raw complement helpers.
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/ace16_preserved_scalar_updates_contract/fixed_scalar_sources/model.rs"]
mod fixed_scalar_model;

// Reader-only fixture members remain exercised by the native SOURCE owner.
#[path = "preserved_boolean_updates/joint_api.rs"]
mod joint_api;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/ace16_preserved_scalar_updates_contract/joint_fixed_sources/model.rs"]
mod joint_fixed_model;
