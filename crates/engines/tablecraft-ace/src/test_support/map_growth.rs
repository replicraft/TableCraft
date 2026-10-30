//! Reuse the independent literal map-growth source for private audit forgeries.
use super::fixed_split::model as fixed_model;
use tablecraft_testkit::access as support;
use tablecraft_testkit::access::mutation_fixture as fixture;

#[allow(dead_code)]
#[path = "../../tests/ace16_index_map_growth_contract/model.rs"]
pub(crate) mod model;
