#[cfg(feature = "ace16-write-preview")]
pub(super) mod spans;

pub(super) trait BytePatch {
    fn offset(&self) -> usize;
    fn before(&self) -> &[u8];
    fn after(&self) -> &[u8];
    fn requires_structural_check(&self) -> bool {
        self.is_mask()
    }
    fn is_mask(&self) -> bool {
        false
    }
}
