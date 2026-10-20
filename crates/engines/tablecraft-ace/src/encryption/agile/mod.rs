//! Bounded AES-CBC Agile password verification and explicit page transforms.
mod crypto;
mod descriptor;
use super::{EncryptionInspection, OfficeEncryptionFamily};
use crate::header::{ENCODED_ZERO_KEY, ENCODING_KEY_RANGE};
use crate::{Header, InspectError, format::page::PAGE_BYTES, header::check};
pub use crypto::AgilePageDecoder;
use std::io::{Read, Seek};
use tablecraft_core::limits::{Resource, ResourceBudget};
use zeroize::Zeroizing;

const AES_BLOCK_BYTES: usize = 16;
const MAX_HASH_BYTES: usize = 64;
const MAX_KEY_BYTES: usize = 32;
const MAX_SPIN_COUNT: u32 = 10_000_000;
const KEY_PADDING: u8 = 0x36;

/// Qualified Agile hash algorithms. These identify native compatibility schemes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgileHashAlgorithm {
    /// Legacy SHA-1 used by existing Office encryption profiles.
    Sha1,
    /// SHA-256.
    Sha256,
    /// SHA-384.
    Sha384,
    /// SHA-512.
    Sha512,
}
impl AgileHashAlgorithm {
    fn bytes(self) -> usize {
        match self {
            Self::Sha1 => 20,
            Self::Sha256 => 32,
            Self::Sha384 => 48,
            Self::Sha512 => 64,
        }
    }
}

/// Validated, non-secret AES-CBC configuration; does not include salts or ciphertext.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AgileParameters {
    data_key_bits: u16,
    password_key_bits: u16,
    data_hash: AgileHashAlgorithm,
    password_hash: AgileHashAlgorithm,
    spin_count: u32,
    has_integrity: bool,
}
impl AgileParameters {
    /// AES key width for the data pages.
    pub const fn data_key_bits(self) -> u16 {
        self.data_key_bits
    }
    /// AES key width for the password-encrypted verifier and intermediate key.
    pub const fn password_key_bits(self) -> u16 {
        self.password_key_bits
    }
    /// Hash algorithm used for native data-page initialization vectors.
    pub const fn data_hash(self) -> AgileHashAlgorithm {
        self.data_hash
    }
    /// Hash algorithm used for password key derivation and verification.
    pub const fn password_hash(self) -> AgileHashAlgorithm {
        self.password_hash
    }
    /// Exact declared iteration count; work is charged during unlocking.
    pub const fn spin_count(self) -> u32 {
        self.spin_count
    }
    /// Whether a data-integrity element exists. Its HMAC is not verified by this codec.
    pub const fn has_integrity(self) -> bool {
        self.has_integrity
    }
}

struct CipherParameters {
    key_bits: u16,
    hash: AgileHashAlgorithm,
    salt: [u8; AES_BLOCK_BYTES],
}
struct Descriptor {
    data: CipherParameters,
    password: CipherParameters,
    spins: u32,
    verifier: [u8; AES_BLOCK_BYTES],
    verifier_hash: [u8; MAX_HASH_BYTES],
    key: [u8; MAX_KEY_BYTES],
    has_integrity: bool,
}

/// Validated AES-CBC Agile descriptor, retaining no plaintext password or data key.
/// Does not implement Debug/Clone/serialization, so ciphertext payloads stay out of diagnostics.
pub struct AgileEncryption {
    descriptor: Descriptor,
    encoding_key: Zeroizing<[u8; 4]>,
    page_count: u32,
}
impl AgileEncryption {
    /// Non-secret configuration, safe for host presentation.
    pub fn parameters(&self) -> AgileParameters {
        let d = &self.descriptor;
        AgileParameters {
            data_key_bits: d.data.key_bits,
            password_key_bits: d.password.key_bits,
            data_hash: d.data.hash,
            password_hash: d.password.hash,
            spin_count: d.spins,
            has_integrity: d.has_integrity,
        }
    }
    /// Verify explicit credentials and recover a source-specific page decoder.
    /// `None` returns PasswordRequired; an explicitly empty password is attempted.
    /// At most 255 UTF-16 code units are accepted, without truncation or normalization.
    /// Iterations consume the caller's cumulative step allowance and check cancellation.
    pub fn unlock(
        &self,
        password: Option<&str>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<AgilePageDecoder, InspectError> {
        crypto::unlock(self, password, budget, cancelled)
    }
}

/// Read and validate an AES-CBC Agile descriptor from a stable/offline source.
/// Accepts the Office envelope profiles documented by inspect_encryption, but
/// does not grant table-layout support. Standard/RC4/external providers and other
/// algorithms return UnsupportedEncryption. No credentials are used until unlock.
pub fn read_agile_encryption<R: Read + Seek>(
    source: &mut R,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<AgileEncryption, InspectError> {
    check(budget, cancelled)?;
    // The envelope inspector retains its narrow metadata-only contract.
    match super::inspect_encryption(source, budget, cancelled)? {
        EncryptionInspection::Office(info) if info.family() == OfficeEncryptionFamily::Agile => {}
        _ => return Err(InspectError::UnsupportedEncryption),
    }
    budget.charge(Resource::Bytes, PAGE_BYTES as u64)?;
    let mut page = Zeroizing::new([0; PAGE_BYTES]);
    tablecraft_io::read_exact_at(source, 0, &mut *page, budget, cancelled)?;
    // Revalidate the exact header being used; callers must still provide a stable source.
    let length = source.seek(std::io::SeekFrom::End(0))?;
    let header = Header::parse(&*page, length)?;
    let info = super::parse_office(&*page)?;
    if !header.encoded_pages()
        || matches!(
            header.format(),
            crate::FileFormat::Jet3 | crate::FileFormat::Jet4
        )
        || header.subversion() > super::OFFICE_ENVELOPE_SUBVERSION
        || info.family() != OfficeEncryptionFamily::Agile
    {
        return Err(InspectError::UnsupportedEncryption);
    }
    let xml_start = super::OFFICE_INFO_OFFSET + super::FLAGS_END;
    let xml_end = super::OFFICE_INFO_OFFSET + usize::from(info.descriptor_bytes());
    let descriptor = descriptor::parse(&page[xml_start..xml_end], budget, cancelled)?;
    let mut encoding_key = Zeroizing::new([0; 4]);
    for (i, byte) in encoding_key.iter_mut().enumerate() {
        *byte = page[ENCODING_KEY_RANGE.start + i] ^ ENCODED_ZERO_KEY[i];
    }
    check(budget, cancelled)?;
    Ok(AgileEncryption {
        descriptor,
        encoding_key,
        page_count: header.page_count(),
    })
}

#[cfg(test)]
mod tests;
