mod composite;
mod encoding;
mod integral;
mod kind;
use super::composite::MixedValue;
pub(in crate::table) use integral::IntegralKey;
pub(in crate::table) use kind::MutationKey;
mod plan;
mod profile;
mod repack;
mod tree;
pub(in crate::table) use plan::patches;

pub(in crate::table) struct IndexKeyUpdate {
    pub reference: u32,
    pub column: usize,
    pub expected: Option<MixedValue>,
    pub value: Option<MixedValue>,
}
pub(in crate::table) struct IndexDataPatch {
    pub offset: usize,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}
