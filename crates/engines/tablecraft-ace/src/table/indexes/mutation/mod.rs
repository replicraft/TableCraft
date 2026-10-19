mod composite;
mod encoding;
mod integral;
pub(in crate::table) use integral::IntegralKey;
mod plan;
mod profile;
mod repack;
mod tree;
pub(in crate::table) use plan::patches;

pub(in crate::table) struct IntegralKeyUpdate {
    pub reference: u32,
    pub expected: Option<i64>,
    pub value: Option<i64>,
}
pub(in crate::table) struct IndexDataPatch {
    pub offset: usize,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}
