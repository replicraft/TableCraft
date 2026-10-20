//! Legacy native interoperability, including explicit Jet 4 encoding creation.
mod crypto;
mod descriptor;
use crate::{Header, InspectError, header::check};
use std::io::{Read, Seek};
use tablecraft_core::limits::{Resource, ResourceBudget};
use zeroize::Zeroizing;

const SALT_BYTES: usize = 16;
const SHA1_BYTES: usize = 20;
const MD5_BYTES: usize = 16;
const MAX_KEY_BYTES: usize = 32;
const STANDARD_ITERATIONS: u32 = 50_000;
const AES_BLOCK_BYTES: usize = 16;

#[derive(Clone, Copy)]
enum Algorithm {
    Jet,
    BinaryRc4,
    CryptoApiRc4,
    Aes { iterations: u32 },
}

pub(crate) struct Encryption {
    algorithm: Algorithm,
    key_bytes: usize,
    salt: [u8; SALT_BYTES],
    verifier: [u8; SALT_BYTES],
    verifier_hash: [u8; MAX_KEY_BYTES],
    encoding_key: Zeroizing<[u8; 4]>,
    header: Header,
    offsets: Option<[usize; 3]>,
}

pub(crate) struct Decoder {
    algorithm: Algorithm,
    key_bytes: usize,
    base: Zeroizing<[u8; MAX_KEY_BYTES]>,
    encoding_key: Zeroizing<[u8; 4]>,
    header: Header,
}

pub(crate) fn read<R: Read + Seek>(
    source: &mut R,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Encryption, InspectError> {
    let header = crate::inspect_header(source, budget, cancelled)?;
    header.decoded().read_profile()?;
    if !header.encoded_pages() {
        return Err(InspectError::UnsupportedEncryption);
    }
    budget.charge(Resource::Bytes, crate::format::page::PAGE_BYTES as u64)?;
    let mut page = Zeroizing::new([0; crate::format::page::PAGE_BYTES]);
    tablecraft_io::read_exact_at(source, 0, &mut *page, budget, cancelled)?;
    let result = descriptor::parse(&*page, header)?;
    check(budget, cancelled)?;
    Ok(result)
}

impl Encryption {
    pub(crate) fn unlock(
        self,
        password: Option<&str>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Decoder, InspectError> {
        crypto::unlock(self, password, budget, cancelled)
    }
}

impl Decoder {
    pub(crate) fn decode_page(
        &self,
        number: u32,
        page: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        crypto::transform(self, number, page, budget, cancelled, false)
    }
}

impl Decoder {
    pub(crate) fn encode_page(
        &self,
        number: u32,
        page: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        crypto::transform(self, number, page, budget, cancelled, true)
    }
}
pub(crate) fn change_password<R: Read + Seek>(
    source: &mut R,
    page: &mut [u8],
    password: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Decoder, InspectError> {
    let mut encryption = read(source, budget, cancelled)?;
    let offsets = encryption
        .offsets
        .ok_or(InspectError::UnsupportedEncryption)?;
    let decoder = crypto::rekey(&mut encryption, password, budget, cancelled)?;
    for (offset, value) in offsets.into_iter().zip([
        &encryption.salt[..],
        &encryption.verifier[..],
        &encryption.verifier_hash[..if matches!(encryption.algorithm, Algorithm::BinaryRc4) {
            MD5_BYTES
        } else if matches!(encryption.algorithm, Algorithm::Aes { .. }) {
            MAX_KEY_BYTES
        } else {
            SHA1_BYTES
        }],
    ]) {
        page.get_mut(offset..offset + value.len())
            .ok_or(InspectError::MalformedEncryption)?
            .copy_from_slice(value);
    }
    Ok(decoder)
}

#[cfg(test)]
mod tests;
