//! Every clear root byte belongs to this admitted header grammar.
use super::refused;
use crate::{
    EditError, InspectError, NativeImage,
    format::{header_mask, page::PAGE_BYTES},
    header::{FORMAT_CODE_OFFSET, check},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const ROOT_SIGNATURE: [u8; 4] = [0, 1, 0, 0];
const ENGINE_OFFSET: usize = ROOT_SIGNATURE.len();
const ENGINE_NAME: &[u8] = b"Standard ACE DB\0";
const WINDOWS_CODE_PAGE: u16 = 1252;
const CODE_PAGE_OFFSET: usize = 60;
const SORT_OFFSET: usize = 110;
const GENERAL_SORT: [u8; 4] = [9, 4, 0, 1];

pub(super) fn validate(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    budget
        .charge(
            Resource::Steps,
            (2 * PAGE_BYTES + header_mask::PREFIX_END) as u64,
        )
        .map_err(InspectError::from)?;
    let mut clear = [0; PAGE_BYTES];
    clear.copy_from_slice(image.as_bytes().get(..PAGE_BYTES).ok_or_else(refused)?);
    header_mask::apply(&mut clear)?;
    let version = clear[FORMAT_CODE_OFFSET];
    if !matches!(version, 5 | 6) {
        return Err(refused());
    }
    // This is a field grammar, not a creator/image digest. Unknown clear header
    // values are refused; names, pages, schemas and data are validated elsewhere.
    let mut expected = [0; PAGE_BYTES];
    expected[..ROOT_SIGNATURE.len()].copy_from_slice(&ROOT_SIGNATURE);
    expected[ENGINE_OFFSET..ENGINE_OFFSET + ENGINE_NAME.len()].copy_from_slice(ENGINE_NAME);
    expected[FORMAT_CODE_OFFSET] = version;
    expected[CODE_PAGE_OFFSET..CODE_PAGE_OFFSET + size_of::<u16>()]
        .copy_from_slice(&WINDOWS_CODE_PAGE.to_le_bytes());
    expected[SORT_OFFSET..SORT_OFFSET + GENERAL_SORT.len()].copy_from_slice(&GENERAL_SORT);
    if clear != expected {
        return Err(refused());
    }
    check(budget, cancelled)?;
    Ok(())
}
