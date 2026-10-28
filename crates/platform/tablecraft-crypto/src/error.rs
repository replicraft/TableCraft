use thiserror::Error;

/// Invalid primitive geometry, without keys, plaintext or ciphertext payloads.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum CryptoError {
    /// The selected cipher cannot use the supplied key length.
    #[error("invalid cipher key length")]
    KeyLength,
    /// A no-padding block operation requires complete AES blocks.
    #[error("invalid block cipher data length")]
    BlockLength,
}
