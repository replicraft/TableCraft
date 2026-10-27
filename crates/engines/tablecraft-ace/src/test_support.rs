//! Independent raw fixtures shared by private unit-test modules.
#[allow(dead_code)]
#[path = "../tests/mutation/fixture.rs"]
pub(crate) mod base;

#[allow(dead_code)]
#[path = "../tests/support/mod.rs"]
pub(crate) mod support;

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
pub(crate) use crate as engine;
