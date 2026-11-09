#[cfg(feature = "ace16-write-preview")]
mod combined;
#[cfg(feature = "journal")]
pub(in crate::table) mod normal_binary_update;
#[cfg(feature = "journal")]
pub(in crate::table) mod normal_fitting;
#[cfg(feature = "journal")]
pub(in crate::table) mod normal_guid_update;
#[cfg(feature = "journal")]
pub(in crate::table) mod normal_index_audit;
#[cfg(feature = "journal")]
pub(in crate::table) mod normal_text_update;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) use combined::index_delete_update_patches;
mod composite;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) mod create;
#[cfg(feature = "ace16-write-preview")]
mod delete;
mod encoding;
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
mod ignored;
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
pub(in crate::table) mod insertion;
mod kind;
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
mod pages;
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
mod population;
use super::composite::DecimalValue;
pub(in crate::table) use super::numeric::NumericKey;
pub(in crate::table) use kind::MutationKey;
#[cfg(feature = "journal")]
use tablecraft_core::values::Guid;
mod plan;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) mod split;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) use plan::splitting_patches;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) use split::{
    Plan as SplitIndexPlan, continue_slot_patches, qualify_slot as qualify_split_slot_indexes,
    slot_patches as split_slot_patches, supported_scalar as split_scalar_supported,
};
mod profile;
mod repack;
#[cfg(feature = "ace16-write-preview")]
mod replacement;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) use replacement::{
    RowKeyUpdate, SlotKeyChanges, SlotKeyUpdate, patches as slot_patches,
    qualify as qualify_slot_indexes, qualify_row_indexes, row_patches,
};
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
mod retained;
#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
pub(in crate::table) mod retirement;
#[cfg(any(feature = "journal", feature = "ace16-write-preview"))]
mod row_key;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) use retirement::{
    RetiredPage, RetirementTable, audit_global, audit_retirements, compose,
};
mod tree;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) use plan::delete_patches;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) use plan::fixed_patches;
pub(in crate::table) use plan::patches;
#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) use tree::audit_siblings;

pub(in crate::table) struct IndexKeyUpdate {
    pub reference: u32,
    pub column: usize,
    pub expected: Option<DecimalValue>,
    pub value: Option<DecimalValue>,
}

#[cfg(feature = "journal")]
impl IndexKeyUpdate {
    /// Canonical GUID components for the actual typed key owner. Native DATA
    /// byte order belongs to the separate fixed-record encoder.
    pub(in crate::table) fn canonical_guid(
        reference: u32,
        column: usize,
        expected: Option<[u8; Guid::BYTE_LEN]>,
        value: Option<[u8; Guid::BYTE_LEN]>,
    ) -> Self {
        Self {
            reference,
            column,
            expected: expected.map(DecimalValue::Guid),
            value: value.map(DecimalValue::Guid),
        }
    }
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

#[cfg(any(feature = "ace16-write-preview", feature = "journal"))]
mod leaf_removal;

#[cfg(feature = "journal")]
pub(in crate::table) mod normal_deletion;

#[cfg(feature = "journal")]
pub(in crate::table) mod normal_integral_update;
