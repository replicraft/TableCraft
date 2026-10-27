//! Repository-owned tuple fixtures shared by private read and mutation tests.
use super::{base, support};

#[allow(dead_code)]
#[path = "../../tests/composite_indexes/fixture.rs"]
pub(crate) mod numeric_fixture;

#[allow(dead_code)]
#[path = "../../tests/decimal_composite_indexes/fixture.rs"]
pub(crate) mod decimal_fixture;

#[cfg(feature = "ace16-write-preview")]
#[allow(dead_code)]
#[path = "../../tests/guid_composite_indexes/fixture.rs"]
pub(crate) mod mixed_fixture;
