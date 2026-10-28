//! Jet 4 / ACE root-page obfuscation, independent of data-page encryption.
use crate::InspectError;
use tablecraft_crypto::Rc4Stream;

const MASK_START: usize = 24;
pub(crate) const PREFIX_END: usize = MASK_START + 128;
const MASK_KEY: [u8; 4] = [0xc7, 0xda, 0x39, 0x6b];

/// The same XOR transform masks and unmasks the fixed root prefix.
pub(crate) fn apply(bytes: &mut [u8]) -> Result<(), InspectError> {
    let masked = bytes
        .get_mut(MASK_START..PREFIX_END)
        .ok_or(InspectError::Truncated)?;
    Rc4Stream::new(&MASK_KEY)
        .map_err(|_| InspectError::Malformed)?
        .apply(masked);
    Ok(())
}
