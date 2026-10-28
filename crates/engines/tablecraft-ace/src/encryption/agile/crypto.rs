use super::{
    AES_BLOCK_BYTES, AgileEncryption, AgileHashAlgorithm, CipherParameters, KEY_PADDING,
    MAX_HASH_BYTES, MAX_KEY_BYTES,
};
use crate::{InspectError, format::page::PAGE_BYTES, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_crypto::{CryptoError, Direction, HashAlgorithm, aes_cbc, constant_time_eq};
use zeroize::{Zeroize, Zeroizing};

const MAX_PASSWORD_UNITS: usize = 255;
const PASSWORD_BYTES: usize = MAX_PASSWORD_UNITS * size_of::<u16>();
const UNLOCK_SCRATCH_BYTES: u64 = 4 * 1024;
pub(super) const VERIFIER_INPUT_BLOCK: [u8; 8] = [0xfe, 0xa7, 0xd2, 0x76, 0x3b, 0x4b, 0x9e, 0x79];
pub(super) const VERIFIER_HASH_BLOCK: [u8; 8] = [0xd7, 0xaa, 0x0f, 0x6d, 0x30, 0x61, 0x34, 0x4e];
pub(super) const KEY_VALUE_BLOCK: [u8; 8] = [0x14, 0x6e, 0x0b, 0xe7, 0xab, 0xac, 0xd0, 0xd6];

/// Password-verified, source-specific AES-CBC page decoder.
///
/// Owned keys and scratch plaintext are zeroized on drop. This type has no
/// Debug/Clone/serialization or write interface. It does not verify ciphertext
/// authenticity or file HMACs: callers must still validate every decoded page.
/// Raw header inspection does not authenticate; explicit decoded sources use this codec.
pub struct AgilePageDecoder {
    pub(super) key: Zeroizing<[u8; MAX_KEY_BYTES]>,
    encoding_key: Zeroizing<[u8; 4]>,
    params: CipherParameters,
    page_count: u32,
}
impl AgilePageDecoder {
    /// Decode one exact 4096-byte native data page in place, atomically on success.
    /// Page zero is header-masked, not encrypted, and is refused here. The ordinal
    /// must be inside the original source. Bytes/steps are charged cumulatively;
    /// cancellation or any failure leaves the caller's ciphertext unchanged.
    /// No file writes occur and no table-layout qualification is implied.
    pub fn decode_page(
        &self,
        page_number: u32,
        page: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        self.transform(page_number, page, budget, cancelled, false)
    }
    pub(crate) fn encode_page(
        &self,
        page_number: u32,
        page: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        self.transform(page_number, page, budget, cancelled, true)
    }
    fn transform(
        &self,
        page_number: u32,
        page: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
        encrypting: bool,
    ) -> Result<(), InspectError> {
        check(budget, cancelled)?;
        if page_number == 0 || page_number >= self.page_count || page.len() != PAGE_BYTES {
            return Err(InspectError::MalformedEncryption);
        }
        budget.charge(Resource::Bytes, PAGE_BYTES as u64)?;
        budget.charge(Resource::Steps, (PAGE_BYTES / AES_BLOCK_BYTES) as u64)?;
        let mut block = Zeroizing::new(page_number.to_le_bytes());
        for (b, key) in block.iter_mut().zip(self.encoding_key.iter()) {
            *b ^= key;
        }
        // Access adds its native encoding key to the Office segment number.
        let hash = digest(self.params.hash, &[&self.params.salt, &*block]);
        let iv: &[u8; AES_BLOCK_BYTES] = hash[..AES_BLOCK_BYTES].try_into().unwrap();
        let mut decoded = Zeroizing::new([0; PAGE_BYTES]);
        decoded.copy_from_slice(page);
        if encrypting {
            super::rewrite::encrypt(
                &CipherParameters {
                    key_bits: self.params.key_bits,
                    hash: self.params.hash,
                    salt: *iv,
                },
                &self.key,
                &mut *decoded,
            )?;
        } else {
            decrypt(
                &self.key[..key_bytes(self.params.key_bits)],
                iv,
                &mut *decoded,
            )?;
        }
        check(budget, cancelled)?;
        page.copy_from_slice(&*decoded);
        Ok(())
    }
}

pub(super) fn encoder(
    key: Zeroizing<[u8; MAX_KEY_BYTES]>,
    params: CipherParameters,
    encoding_key: [u8; 4],
    page_count: u32,
) -> AgilePageDecoder {
    AgilePageDecoder {
        key,
        params,
        encoding_key: Zeroizing::new(encoding_key),
        page_count,
    }
}

pub(super) fn unlock(
    encryption: &AgileEncryption,
    password: Option<&str>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<AgilePageDecoder, InspectError> {
    check(budget, cancelled)?;
    let password = password.ok_or(InspectError::PasswordRequired)?;
    let d = &encryption.descriptor;
    let p = &d.password;
    let iterated = password_hash(p, password, d.spins, budget, cancelled)?;
    let input_key = derive(p, &iterated, &VERIFIER_INPUT_BLOCK);
    let mut verifier = Zeroizing::new(d.verifier);
    decrypt(&input_key[..key_bytes(p.key_bits)], &p.salt, &mut *verifier)?;
    check(budget, cancelled)?;
    let hash_key = derive(p, &iterated, &VERIFIER_HASH_BLOCK);
    let mut verifier_hash = Zeroizing::new(d.verifier_hash);
    let hash_length = p.hash.bytes().next_multiple_of(AES_BLOCK_BYTES);
    decrypt(
        &hash_key[..key_bytes(p.key_bits)],
        &p.salt,
        &mut verifier_hash[..hash_length],
    )?;
    let expected = digest(p.hash, &[&*verifier]);
    if !constant_time_eq(
        &expected[..p.hash.bytes()],
        &verifier_hash[..p.hash.bytes()],
    ) {
        return Err(InspectError::IncorrectPassword);
    }
    check(budget, cancelled)?;
    let key_key = derive(p, &iterated, &KEY_VALUE_BLOCK);
    let mut key = Zeroizing::new(d.key);
    let encrypted_length = key_bytes(d.data.key_bits).next_multiple_of(AES_BLOCK_BYTES);
    decrypt(
        &key_key[..key_bytes(p.key_bits)],
        &p.salt,
        &mut key[..encrypted_length],
    )?;
    check(budget, cancelled)?;
    Ok(AgilePageDecoder {
        key,
        encoding_key: Zeroizing::new(*encryption.encoding_key),
        params: CipherParameters {
            key_bits: d.data.key_bits,
            hash: d.data.hash,
            salt: d.data.salt,
        },
        page_count: encryption.page_count,
    })
}

pub(super) fn password_hash(
    p: &CipherParameters,
    password: &str,
    spins: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Zeroizing<[u8; MAX_HASH_BYTES]>, InspectError> {
    let mut encoded = Zeroizing::new([0; PASSWORD_BYTES]);
    let mut length = 0;
    for unit in password.encode_utf16() {
        if length == encoded.len() {
            return Err(InspectError::InvalidPasswordLength);
        }
        encoded[length..length + size_of::<u16>()].copy_from_slice(&unit.to_le_bytes());
        length += size_of::<u16>();
    }
    // Fixed owned working storage, independent of the untrusted spin count.
    budget.charge(Resource::Bytes, UNLOCK_SCRATCH_BYTES)?;
    let mut iterated = digest(p.hash, &[&p.salt, &encoded[..length]]);
    for iteration in 0..spins {
        check(budget, cancelled)?;
        let next = digest(
            p.hash,
            &[&iteration.to_le_bytes(), &iterated[..p.hash.bytes()]],
        );
        iterated.zeroize();
        iterated = next;
    }
    Ok(iterated)
}

pub(super) fn key_bytes(bits: u16) -> usize {
    usize::from(bits) / u8::BITS as usize
}

pub(super) fn derive(
    p: &CipherParameters,
    hash: &[u8; MAX_HASH_BYTES],
    block: &[u8; 8],
) -> Zeroizing<[u8; MAX_KEY_BYTES]> {
    let derived = digest(p.hash, &[&hash[..p.hash.bytes()], block]);
    let mut key = Zeroizing::new([KEY_PADDING; MAX_KEY_BYTES]);
    let bytes = MAX_KEY_BYTES.min(p.hash.bytes());
    key[..bytes].copy_from_slice(&derived[..bytes]);
    key
}

pub(super) fn digest(
    algorithm: AgileHashAlgorithm,
    parts: &[&[u8]],
) -> Zeroizing<[u8; MAX_HASH_BYTES]> {
    let algorithm = match algorithm {
        AgileHashAlgorithm::Sha1 => HashAlgorithm::Sha1,
        AgileHashAlgorithm::Sha256 => HashAlgorithm::Sha256,
        AgileHashAlgorithm::Sha384 => HashAlgorithm::Sha384,
        AgileHashAlgorithm::Sha512 => HashAlgorithm::Sha512,
    };
    tablecraft_crypto::digest(algorithm, parts)
}

fn decrypt(key: &[u8], iv: &[u8; AES_BLOCK_BYTES], data: &mut [u8]) -> Result<(), InspectError> {
    aes_cbc(key, iv, data, Direction::Decrypt).map_err(|error| match error {
        CryptoError::KeyLength => InspectError::UnsupportedEncryption,
        CryptoError::BlockLength => InspectError::MalformedEncryption,
    })
}

#[cfg(test)]
mod tests;
