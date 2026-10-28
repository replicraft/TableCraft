use crate::CryptoError;
use aes::cipher::{
    BlockCipherDecrypt, BlockCipherEncrypt, BlockModeDecrypt, BlockModeEncrypt, KeyInit, KeyIvInit,
    block_padding::NoPadding,
};

/// AES block width, independent of a native database's page geometry.
pub const AES_BLOCK_BYTES: usize = 16;

/// Explicit block-operation direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Encrypt complete blocks.
    Encrypt,
    /// Decrypt complete blocks.
    Decrypt,
}

/// Transform complete AES-ECB blocks in place without padding or tail policy.
/// Invalid key/data geometry is rejected before mutation. Callers own bounded
/// work, cancellation and any atomic publication through separate scratch.
pub fn aes_ecb(key: &[u8], bytes: &mut [u8], direction: Direction) -> Result<(), CryptoError> {
    validate(key, bytes)?;
    macro_rules! transform {
        ($cipher:ty) => {{
            let cipher = <$cipher>::new_from_slice(key).map_err(|_| CryptoError::KeyLength)?;
            for block in bytes.as_chunks_mut::<AES_BLOCK_BYTES>().0 {
                match direction {
                    Direction::Encrypt => cipher.encrypt_block(block.into()),
                    Direction::Decrypt => cipher.decrypt_block(block.into()),
                }
            }
        }};
    }
    match key.len() {
        16 => transform!(aes::Aes128),
        24 => transform!(aes::Aes192),
        32 => transform!(aes::Aes256),
        _ => return Err(CryptoError::KeyLength),
    }
    Ok(())
}

/// Transform complete AES-CBC blocks with an explicit IV and no padding.
/// Invalid key/data geometry is rejected before mutation. This does not choose
/// IV derivation, certify authenticity or provide a file encryption format.
pub fn aes_cbc(
    key: &[u8],
    iv: &[u8; AES_BLOCK_BYTES],
    bytes: &mut [u8],
    direction: Direction,
) -> Result<(), CryptoError> {
    validate(key, bytes)?;
    macro_rules! transform {
        ($cipher:ty) => {{
            match direction {
                Direction::Encrypt => {
                    let length = bytes.len();
                    cbc::Encryptor::<$cipher>::new_from_slices(key, iv)
                        .map_err(|_| CryptoError::KeyLength)?
                        .encrypt_padded::<NoPadding>(bytes, length)
                        .map_err(|_| CryptoError::BlockLength)?;
                }
                Direction::Decrypt => {
                    cbc::Decryptor::<$cipher>::new_from_slices(key, iv)
                        .map_err(|_| CryptoError::KeyLength)?
                        .decrypt_padded::<NoPadding>(bytes)
                        .map_err(|_| CryptoError::BlockLength)?;
                }
            }
        }};
    }
    match key.len() {
        16 => transform!(aes::Aes128),
        24 => transform!(aes::Aes192),
        32 => transform!(aes::Aes256),
        _ => return Err(CryptoError::KeyLength),
    }
    Ok(())
}

fn validate(key: &[u8], bytes: &[u8]) -> Result<(), CryptoError> {
    if !matches!(key.len(), 16 | 24 | 32) {
        return Err(CryptoError::KeyLength);
    }
    if !bytes.len().is_multiple_of(AES_BLOCK_BYTES) {
        return Err(CryptoError::BlockLength);
    }
    Ok(())
}
