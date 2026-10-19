mod encoding;
mod plan;
mod repack;
mod tree;
pub(in crate::table) use plan::patches;

pub(in crate::table) struct Int32KeyUpdate {
    pub reference: u32,
    pub expected: Option<i32>,
    pub value: Option<i32>,
}
pub(in crate::table) struct IndexDataPatch {
    pub offset: usize,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}
