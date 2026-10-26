mod composite;
#[cfg(feature = "ace16-write-preview")]
mod delete;
mod encoding;
mod kind;
use super::composite::DecimalValue;
pub(in crate::table) use super::numeric::NumericKey;
pub(in crate::table) use kind::MutationKey;
mod plan;
mod profile;
mod repack;
#[cfg(feature = "ace16-write-preview")]
mod retirement;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) use retirement::{RetiredPage, audit_retirements};
mod tree;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) use plan::delete_patches;
pub(in crate::table) use plan::patches;

pub(in crate::table) struct IndexKeyUpdate {
    pub reference: u32,
    pub column: usize,
    pub expected: Option<DecimalValue>,
    pub value: Option<DecimalValue>,
}
pub(in crate::table) struct IndexDataPatch {
    pub offset: usize,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}

#[derive(Default)]
pub(in crate::table) struct PatchSet {
    pub indexes: Vec<IndexDataPatch>,
    #[cfg(feature = "ace16-write-preview")]
    pub retired: Vec<RetiredPage>,
}

#[cfg(test)]
mod numeric_tests;
