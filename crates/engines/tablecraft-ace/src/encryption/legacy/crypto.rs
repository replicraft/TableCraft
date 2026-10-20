use super::{
    AES_BLOCK_BYTES, Algorithm, Decoder, Encryption, MAX_KEY_BYTES, MD5_BYTES, SHA1_BYTES,
};
use crate::{InspectError, format::page::PAGE_BYTES, header::check};
use aes::cipher::{BlockCipherDecrypt, KeyInit};
use rc4::StreamCipher;
use sha2::Digest;
use subtle::ConstantTimeEq;
use tablecraft_core::limits::{Resource, ResourceBudget};
use zeroize::{Zeroize, Zeroizing};

const MAX_PASSWORD_UNITS: usize = 255;
const PASSWORD_BYTES: usize = MAX_PASSWORD_UNITS * size_of::<u16>();
const BINARY_PREFIX_BYTES: usize = 5;
const BINARY_REPEAT_BYTES: usize = 336;
const DIGEST_BLOCK_BYTES: usize = 64;
const INNER_PAD: u8 = 0x36;
const OUTER_PAD: u8 = 0x5c;
const SCRATCH_BYTES: u64 = 4096;

pub(super) fn unlock(
    encryption: Encryption,
    password: Option<&str>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Decoder, InspectError> {
    check(budget, cancelled)?;
    budget.charge(Resource::Bytes, SCRATCH_BYTES)?;
    let mut base = Zeroizing::new([0; MAX_KEY_BYTES]);
    if !matches!(encryption.algorithm, Algorithm::Jet) {
        let password = password.ok_or(InspectError::PasswordRequired)?;
        let mut bytes = Zeroizing::new([0; PASSWORD_BYTES]);
        let mut length = 0;
        for unit in password.encode_utf16() {
            if length == bytes.len() {
                return Err(InspectError::InvalidPasswordLength);
            }
            bytes[length..length + size_of::<u16>()].copy_from_slice(&unit.to_le_bytes());
            length += size_of::<u16>();
        }
        match encryption.algorithm {
            Algorithm::BinaryRc4 => {
                let hash = digest::<md5::Md5>(&[&bytes[..length]]);
                let mut repeated = Zeroizing::new([0; BINARY_REPEAT_BYTES]);
                for chunk in repeated.chunks_exact_mut(BINARY_PREFIX_BYTES + encryption.salt.len())
                {
                    chunk[..BINARY_PREFIX_BYTES].copy_from_slice(&hash[..BINARY_PREFIX_BYTES]);
                    chunk[BINARY_PREFIX_BYTES..].copy_from_slice(&encryption.salt);
                }
                let hash = digest::<md5::Md5>(&[&*repeated]);
                base[..BINARY_PREFIX_BYTES].copy_from_slice(&hash[..BINARY_PREFIX_BYTES]);
            }
            Algorithm::CryptoApiRc4 | Algorithm::Aes { .. } => {
                let mut hash = digest::<sha1::Sha1>(&[&encryption.salt, &bytes[..length]]);
                if let Algorithm::Aes { iterations } = encryption.algorithm {
                    for i in 0..iterations {
                        check(budget, cancelled)?;
                        let next = digest::<sha1::Sha1>(&[&i.to_le_bytes(), &hash[..SHA1_BYTES]]);
                        hash.zeroize();
                        hash = next;
                    }
                }
                base[..SHA1_BYTES].copy_from_slice(&hash[..SHA1_BYTES]);
            }
            Algorithm::Jet => unreachable!(),
        }
    }
    let decoder = Decoder {
        algorithm: encryption.algorithm,
        key_bytes: encryption.key_bytes,
        base,
        encoding_key: encryption.encoding_key,
        header: encryption.header,
    };
    if !matches!(decoder.algorithm, Algorithm::Jet) {
        let key = key(&decoder, &[0; 4]);
        let mut verifier = Zeroizing::new(encryption.verifier);
        let mut actual = Zeroizing::new(encryption.verifier_hash);
        let hash_bytes = if matches!(decoder.algorithm, Algorithm::BinaryRc4) {
            MD5_BYTES
        } else {
            SHA1_BYTES
        };
        if matches!(decoder.algorithm, Algorithm::Aes { .. }) {
            aes_decrypt(&key[..decoder.key_bytes], &mut *verifier)?;
            aes_decrypt(&key[..decoder.key_bytes], &mut *actual)?;
        } else {
            let mut cipher = rc4::Rc4::new_from_slice(&key[..stream_key_bytes(&decoder)])
                .map_err(|_| InspectError::MalformedEncryption)?;
            cipher.apply_keystream(&mut *verifier);
            cipher.apply_keystream(&mut actual[..hash_bytes]);
        }
        let expected = if matches!(decoder.algorithm, Algorithm::BinaryRc4) {
            digest::<md5::Md5>(&[&*verifier])
        } else {
            digest::<sha1::Sha1>(&[&*verifier])
        };
        if !bool::from(expected[..hash_bytes].ct_eq(&actual[..hash_bytes])) {
            return Err(InspectError::IncorrectPassword);
        }
    }
    check(budget, cancelled)?;
    Ok(decoder)
}

fn stream_key_bytes(decoder: &Decoder) -> usize {
    if matches!(decoder.algorithm, Algorithm::CryptoApiRc4)
        && decoder.key_bytes == BINARY_PREFIX_BYTES
    {
        MD5_BYTES
    } else {
        decoder.key_bytes
    }
}

fn key(decoder: &Decoder, block: &[u8; 4]) -> Zeroizing<[u8; MAX_KEY_BYTES]> {
    match decoder.algorithm {
        Algorithm::Jet => {
            let mut key = Zeroizing::new([0; MAX_KEY_BYTES]);
            key[..block.len()].copy_from_slice(block);
            key
        }
        Algorithm::BinaryRc4 => digest::<md5::Md5>(&[&decoder.base[..BINARY_PREFIX_BYTES], block]),
        Algorithm::CryptoApiRc4 => {
            let mut key = digest::<sha1::Sha1>(&[&decoder.base[..SHA1_BYTES], block]);
            key[decoder.key_bytes..].zeroize();
            key
        }
        Algorithm::Aes { .. } => {
            let hash = digest::<sha1::Sha1>(&[&decoder.base[..SHA1_BYTES], block]);
            let mut pads = Zeroizing::new([
                [INNER_PAD; DIGEST_BLOCK_BYTES],
                [OUTER_PAD; DIGEST_BLOCK_BYTES],
            ]);
            for pad in pads.iter_mut() {
                for (b, h) in pad.iter_mut().zip(hash[..SHA1_BYTES].iter()) {
                    *b ^= h;
                }
            }
            let first = digest::<sha1::Sha1>(&[&pads[0]]);
            let second = digest::<sha1::Sha1>(&[&pads[1]]);
            let mut key = Zeroizing::new([0; MAX_KEY_BYTES]);
            key[..SHA1_BYTES].copy_from_slice(&first[..SHA1_BYTES]);
            key[SHA1_BYTES..].copy_from_slice(&second[..MAX_KEY_BYTES - SHA1_BYTES]);
            key
        }
    }
}

pub(super) fn decode(
    decoder: &Decoder,
    number: u32,
    page: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    check(budget, cancelled)?;
    if number == 0 || number >= decoder.header.page_count() || page.len() != PAGE_BYTES {
        return Err(InspectError::MalformedEncryption);
    }
    budget.charge(Resource::Bytes, PAGE_BYTES as u64)?;
    budget.charge(Resource::Steps, (PAGE_BYTES / AES_BLOCK_BYTES) as u64)?;
    let mut block = Zeroizing::new(number.to_le_bytes());
    for (b, key) in block.iter_mut().zip(decoder.encoding_key.iter()) {
        *b ^= key;
    }
    let key = key(decoder, &block);
    let mut scratch = Zeroizing::new([0; PAGE_BYTES]);
    scratch.copy_from_slice(page);
    if matches!(decoder.algorithm, Algorithm::Aes { .. }) {
        aes_decrypt(&key[..decoder.key_bytes], &mut *scratch)?;
    } else {
        rc4::Rc4::new_from_slice(&key[..stream_key_bytes(decoder)])
            .map_err(|_| InspectError::MalformedEncryption)?
            .apply_keystream(&mut *scratch);
    }
    check(budget, cancelled)?;
    page.copy_from_slice(&*scratch);
    Ok(())
}

fn digest<D: Digest>(parts: &[&[u8]]) -> Zeroizing<[u8; MAX_KEY_BYTES]> {
    let mut hasher = D::new();
    for part in parts {
        hasher.update(part);
    }
    let mut hash = hasher.finalize();
    let mut result = Zeroizing::new([0; MAX_KEY_BYTES]);
    result[..hash.len()].copy_from_slice(&hash);
    hash.as_mut_slice().zeroize();
    result
}

fn aes_decrypt(key: &[u8], bytes: &mut [u8]) -> Result<(), InspectError> {
    macro_rules! decrypt {
        ($cipher:ty) => {{
            let cipher =
                <$cipher>::new_from_slice(key).map_err(|_| InspectError::MalformedEncryption)?;
            for block in bytes.as_chunks_mut::<AES_BLOCK_BYTES>().0 {
                cipher.decrypt_block(block.into());
            }
        }};
    }
    match key.len() {
        16 => decrypt!(aes::Aes128),
        24 => decrypt!(aes::Aes192),
        32 => decrypt!(aes::Aes256),
        _ => return Err(InspectError::UnsupportedEncryption),
    }
    Ok(())
}
