mod composite;
#[cfg(feature = "ace16-write-preview")]
mod delete;
mod encoding;
mod kind;
use super::composite::DecimalValue;
pub(in crate::table) use super::numeric::NumericKey;
pub(in crate::table) use kind::MutationKey;
mod plan;
#[cfg(feature = "ace16-write-preview")]
mod split;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) use plan::splitting_patches;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) use split::{
    Plan as SplitIndexPlan, qualify_slot as qualify_split_slot_indexes,
    slot_patches as split_slot_patches, supported_scalar as split_scalar_supported,
};
mod profile;
mod repack;
#[cfg(feature = "ace16-write-preview")]
mod replacement;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) use replacement::{
    SlotKeyChanges, SlotKeyUpdate, patches as slot_patches, qualify as qualify_slot_indexes,
};
#[cfg(feature = "ace16-write-preview")]
mod retained;
#[cfg(feature = "ace16-write-preview")]
mod retirement;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) use retirement::{
    RetiredPage, RetirementTable, audit_global, audit_retirements, compose,
};
mod tree;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) use plan::delete_patches;
pub(in crate::table) use plan::patches;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) use tree::audit_siblings;

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
    #[cfg(feature = "ace16-write-preview")]
    pub siblings: Vec<tree::SiblingAudit>,
}

#[cfg(test)]
mod numeric_tests;
