pub(super) trait BytePatch {
    fn offset(&self) -> usize;
    fn before(&self) -> &[u8];
    fn after(&self) -> &[u8];
    fn is_mask(&self) -> bool {
        false
    }
}
