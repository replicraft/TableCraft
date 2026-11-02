#![cfg(feature = "ace16-write-preview")]

#[allow(dead_code)]
#[path = "ace16_split_int32/model.rs"]
mod base_model;
#[allow(dead_code)]
#[path = "ace16_nonroot_split_int32/model.rs"]
mod model;
use tablecraft_testkit::fixtures::access as support;
#[path = "ace16_nonroot_split_int32/values.rs"]
mod values;

use tablecraft_ace as engine;
