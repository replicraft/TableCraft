#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod base;
#[path = "long_value_storage_contract/model.rs"]
mod model;
#[allow(dead_code)]
use tablecraft_testkit::access as support;
#[path = "long_value_storage_contract/values.rs"]
mod values;
