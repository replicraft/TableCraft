//! Independent raw fixtures shared by private unit-test modules.
// Path-included fixture modules receive their private raw-builder namespace here.
use tablecraft_testkit::access as support;

pub(crate) mod tuples;

#[allow(dead_code)]
#[path = "../tests/mutation/fixture.rs"]
pub(crate) mod base;

#[cfg(feature = "ace16-write-preview")]
pub(crate) use base as fixture;

#[cfg(feature = "ace16-write-preview")]
#[allow(dead_code)]
#[path = "../tests/mutation/index_fixture.rs"]
pub(crate) mod index_fixture;

#[cfg(feature = "ace16-write-preview")]
#[allow(dead_code)]
#[path = "../tests/ace16_split_int32/model.rs"]
pub(crate) mod split_model;

#[cfg(feature = "ace16-write-preview")]
pub(crate) use split_model as base_model;

#[cfg(feature = "ace16-write-preview")]
#[allow(dead_code)]
#[path = "../tests/ace16_nonroot_split_int32/model.rs"]
pub(crate) mod nonroot_model;

#[cfg(feature = "ace16-write-preview")]
pub(crate) use crate as engine;

#[cfg(feature = "ace16-write-preview")]
#[path = "test_support/fixed_split.rs"]
pub(crate) mod fixed_split;

#[cfg(feature = "ace16-write-preview")]
#[path = "test_support/map_growth.rs"]
pub(crate) mod map_growth;

#[cfg(feature = "ace16-write-preview")]
#[path = "test_support/encoded.rs"]
pub(crate) mod encoded;
