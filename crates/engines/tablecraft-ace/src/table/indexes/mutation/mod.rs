mod composite;
mod encoding;
mod kind;
mod numeric;
use super::composite::DecimalValue;
pub(in crate::table) use kind::MutationKey;
pub(in crate::table) use numeric::NumericKey;
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
