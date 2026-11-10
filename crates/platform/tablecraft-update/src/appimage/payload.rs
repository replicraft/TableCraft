use crate::{UpdateError, platform::Platform};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
};

const ELF_HEADER_BYTES: usize = 64;
const ELF_IDENT: &[u8; 7] = b"\x7fELF\x02\x01\x01";
const ELF_OS_ABI_OFFSET: usize = 7;
const APPIMAGE_TAG_OFFSET: usize = 8;
const APPIMAGE_TAG: &[u8; 3] = b"AI\x02";
const ELF_TYPE_OFFSET: usize = 16;
const ELF_MACHINE_OFFSET: usize = 18;
const ELF_EXECUTABLE: u16 = 2;
const ELF_POSITION_INDEPENDENT: u16 = 3;
const ELF_X64: u16 = 62;
const ELF_ARM64: u16 = 183;

/// Inspect only fixed inert Type 2 runtime headers; this never executes an image.
pub(super) fn check(file: &mut File, platform: Platform) -> Result<(), UpdateError> {
    let mut header = [0; ELF_HEADER_BYTES];
    file.seek(SeekFrom::Start(0)).map_err(UpdateError::Io)?;
    file.read_exact(&mut header)
        .map_err(|_| UpdateError::Archive)?;
    let machine = match platform {
        Platform::LinuxX64 => ELF_X64,
        Platform::LinuxArm64 => ELF_ARM64,
        _ => return Err(UpdateError::Archive),
    };
    let file_type = u16::from_le_bytes([header[ELF_TYPE_OFFSET], header[ELF_TYPE_OFFSET + 1]]);
    let actual_machine =
        u16::from_le_bytes([header[ELF_MACHINE_OFFSET], header[ELF_MACHINE_OFFSET + 1]]);
    if !header.starts_with(ELF_IDENT)
        || !matches!(header[ELF_OS_ABI_OFFSET], 0 | 3)
        || &header[APPIMAGE_TAG_OFFSET..APPIMAGE_TAG_OFFSET + APPIMAGE_TAG.len()] != APPIMAGE_TAG
        || !matches!(file_type, ELF_EXECUTABLE | ELF_POSITION_INDEPENDENT)
        || actual_machine != machine
    {
        return Err(UpdateError::Archive);
    }
    file.seek(SeekFrom::Start(0)).map_err(UpdateError::Io)?;
    Ok(())
}
