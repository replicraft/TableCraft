#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
#[path = "ace16_split_int32/model.rs"]
mod base_model;
#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
#[path = "ace16_nonroot_split_int32/model.rs"]
mod model;
use tablecraft_testkit::access as support;
#[path = "ace16_nonroot_split_int32/values.rs"]
mod values;

use tablecraft_ace as engine;
