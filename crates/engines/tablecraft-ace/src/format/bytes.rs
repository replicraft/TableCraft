use crate::InspectError;

pub(crate) fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, InspectError> {
    let b = bytes
        .get(offset..offset + size_of::<u16>())
        .ok_or(InspectError::Truncated)?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}
pub(crate) fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, InspectError> {
    let b = bytes
        .get(offset..offset + size_of::<u32>())
        .ok_or(InspectError::Truncated)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
