mod composite;
mod encoding;
mod kind;
use super::composite::DecimalValue;
pub(in crate::table) use super::numeric::NumericKey;
pub(in crate::table) use kind::MutationKey;
mod plan;
mod profile;
mod repack;
mod tree;
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

#[cfg(test)]
mod numeric_tests;
