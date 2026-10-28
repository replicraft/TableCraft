//! Checked algorithm integration without database, file or password policy.
//!
//! Consumers own resource charging, cancellation, input-size bounds and atomic
//! publication. These prepaid primitives perform one caller-bounded operation;
//! they do not allocate variable-sized buffers or poll external callbacks.
//! Cipher states and digest working/output buffers are zeroized on drop.
//!
//! MD5, SHA-1 and RC4 exist for compatibility consumers. This crate does not
//! select encryption profiles, derive native page keys/IVs, authenticate files,
//! establish trust or implement digital signatures.
#![deny(missing_docs)]

mod aes;
mod error;
mod hash;
mod stream;

pub use aes::{AES_BLOCK_BYTES, Direction, aes_cbc, aes_ecb};
pub use error::CryptoError;
pub use hash::{HashAlgorithm, MAX_DIGEST_BYTES, digest};
pub use stream::Rc4Stream;

/// Compare equal-length contents without content-dependent early termination.
/// Length equality is public and checked separately by the primitive.
pub fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    use subtle::ConstantTimeEq;
    bool::from(left.ct_eq(right))
}
