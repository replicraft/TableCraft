//! SOURCE-only complete large images; no capture-policy or mutation relaxation.
use tablecraft_testkit::fixtures::access::complex_schema as complex_fixture;
use tablecraft_testkit::fixtures::access::mutation_fixture as base;
#[path = "ace16_larger_fixed_images/corrected.rs"]
pub mod corrected;
#[path = "ace16_larger_fixed_images/model.rs"]
pub mod model;
use tablecraft_testkit::fixtures::access::attachment_modern as modern_attachment;
#[path = "ace16_preserved_scalar_updates_contract/model.rs"]
mod preserved_model;
#[path = "ace16_larger_fixed_images/source.rs"]
mod source;

use support as raw;
use tablecraft_testkit::fixtures::access as support;
