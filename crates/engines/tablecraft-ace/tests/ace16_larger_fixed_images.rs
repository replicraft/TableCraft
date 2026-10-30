//! SOURCE-only complete large images; no capture-policy or mutation relaxation.
#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
pub mod base;
#[allow(dead_code)]
#[path = "complex_schema/fixture.rs"]
pub mod complex_fixture;
#[path = "ace16_larger_fixed_images/corrected.rs"]
mod corrected;
#[path = "ace16_larger_fixed_images/model.rs"]
mod model;
#[allow(dead_code)]
#[path = "attachment_modern/fixture.rs"]
pub mod modern_attachment;
#[allow(dead_code)]
#[path = "attachments/fixture.rs"]
pub mod old_attachment;
#[path = "ace16_preserved_scalar_updates_contract/model.rs"]
mod preserved_model;
#[path = "ace16_larger_fixed_images/source.rs"]
mod source;

use support as raw;
use tablecraft_testkit::access as support;
