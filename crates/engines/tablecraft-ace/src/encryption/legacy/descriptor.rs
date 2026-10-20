use super::{
    AES_BLOCK_BYTES, Algorithm, Encryption, MAX_KEY_BYTES, MD5_BYTES, SALT_BYTES, SHA1_BYTES,
    STANDARD_ITERATIONS,
};
use crate::{FileFormat, Header, InspectError};
use zeroize::Zeroizing;

const CRYPTO_API: u32 = 0x04;
const DOCUMENT_PROPERTIES: u32 = 0x08;
const AES_FLAG: u32 = 0x20;
const HEADER_FIELD_COUNT: usize = 8;
const HEADER_FIXED_BYTES: usize = HEADER_FIELD_COUNT * size_of::<u32>();
const PROVIDER_RSA_FULL: u32 = 1;
const PROVIDER_RSA_AES: u32 = 24;
const RC4_MIN_KEY_BITS: u32 = 40;
const RC4_MAX_KEY_BITS: u32 = 128;
const ALG_RC4: u32 = 0x6801;
const ALG_AES128: u32 = 0x660e;
const ALG_AES192: u32 = 0x660f;
const ALG_AES256: u32 = 0x6610;
const HASH_SHA1: u32 = 0x8004;

pub(super) fn parse(page: &[u8], header: Header) -> Result<Encryption, InspectError> {
    let mut result = Encryption {
        algorithm: Algorithm::Jet,
        key_bytes: 4,
        salt: [0; SALT_BYTES],
        verifier: [0; SALT_BYTES],
        verifier_hash: [0; MAX_KEY_BYTES],
        encoding_key: Zeroizing::new([0; 4]),
        header,
    };
    for (i, byte) in result.encoding_key.iter_mut().enumerate() {
        *byte =
            page[crate::header::ENCODING_KEY_RANGE.start + i] ^ crate::header::ENCODED_ZERO_KEY[i];
    }
    if header.format() == FileFormat::Jet4 {
        return Ok(result);
    }
    let info = super::super::parse_office(page)?;
    let mut bytes = &page[super::super::OFFICE_INFO_OFFSET
        ..super::super::OFFICE_INFO_OFFSET + usize::from(info.descriptor_bytes())];
    take(&mut bytes, 2 * size_of::<u16>())?; // Version already validated by the envelope parser.
    if info.family() == super::super::OfficeEncryptionFamily::BinaryRc4 {
        result.algorithm = Algorithm::BinaryRc4;
        result.key_bytes = MD5_BYTES;
        result.salt.copy_from_slice(take(&mut bytes, SALT_BYTES)?);
        result
            .verifier
            .copy_from_slice(take(&mut bytes, SALT_BYTES)?);
        result.verifier_hash[..MD5_BYTES].copy_from_slice(take(&mut bytes, MD5_BYTES)?);
    } else if info.family() == super::super::OfficeEncryptionFamily::StandardOrCryptoApi {
        let flags = word(&mut bytes)?;
        if flags & CRYPTO_API == 0 || flags & !(CRYPTO_API | DOCUMENT_PROPERTIES | AES_FLAG) != 0 {
            return Err(InspectError::UnsupportedEncryption);
        }
        let length =
            usize::try_from(word(&mut bytes)?).map_err(|_| InspectError::MalformedEncryption)?;
        let mut fields = take(&mut bytes, length)?;
        if length < HEADER_FIXED_BYTES || !length.is_multiple_of(size_of::<u16>()) {
            return Err(InspectError::MalformedEncryption);
        }
        if word(&mut fields)? != flags || word(&mut fields)? != 0 {
            return Err(InspectError::UnsupportedEncryption);
        }
        let algorithm = word(&mut fields)?;
        let hash = word(&mut fields)?;
        let key_bits = word(&mut fields)?;
        let provider = word(&mut fields)?;
        if word(&mut fields)? != 0 || word(&mut fields)? != 0 {
            return Err(InspectError::MalformedEncryption);
        }
        if hash != 0 && hash != HASH_SHA1 {
            return Err(InspectError::UnsupportedEncryption);
        }
        // Provider names determine the RC4 default only; never invoke a provider.
        let mut name = String::new();
        for unit in fields.as_chunks::<2>().0 {
            let unit = u16::from_le_bytes(*unit);
            if unit == 0 {
                break;
            }
            name.push(char::from_u32(u32::from(unit)).ok_or(InspectError::MalformedEncryption)?);
        }
        let algorithm = if algorithm == 0 {
            if flags & AES_FLAG != 0 {
                ALG_AES128
            } else {
                ALG_RC4
            }
        } else {
            algorithm
        };
        let bits = match algorithm {
            ALG_RC4 if flags & AES_FLAG == 0 && provider == PROVIDER_RSA_FULL => {
                result.algorithm = Algorithm::CryptoApiRc4;
                let bits = if key_bits == 0 {
                    let name = name.trim().to_ascii_lowercase();
                    if name.is_empty() || name.contains(" base ") {
                        RC4_MIN_KEY_BITS
                    } else {
                        RC4_MAX_KEY_BITS
                    }
                } else {
                    key_bits
                };
                if !(RC4_MIN_KEY_BITS..=RC4_MAX_KEY_BITS).contains(&bits)
                    || !bits.is_multiple_of(u8::BITS)
                {
                    return Err(InspectError::UnsupportedEncryption);
                }
                bits
            }
            ALG_AES128 | ALG_AES192 | ALG_AES256 if provider == PROVIDER_RSA_AES => {
                let bits = match algorithm {
                    ALG_AES128 => 128,
                    ALG_AES192 => 192,
                    _ => 256,
                };
                if key_bits != 0 && key_bits != bits {
                    return Err(InspectError::UnsupportedEncryption);
                }
                result.algorithm = Algorithm::Aes {
                    iterations: if flags & AES_FLAG == 0 {
                        0
                    } else {
                        STANDARD_ITERATIONS
                    },
                };
                bits
            }
            _ => return Err(InspectError::UnsupportedEncryption),
        };
        result.key_bytes = (bits / u8::BITS) as usize;
        if word(&mut bytes)? != SALT_BYTES as u32 {
            return Err(InspectError::MalformedEncryption);
        }
        result.salt.copy_from_slice(take(&mut bytes, SALT_BYTES)?);
        result
            .verifier
            .copy_from_slice(take(&mut bytes, SALT_BYTES)?);
        if word(&mut bytes)? != SHA1_BYTES as u32 {
            return Err(InspectError::MalformedEncryption);
        }
        let hash_bytes = if matches!(result.algorithm, Algorithm::Aes { .. }) {
            SHA1_BYTES.next_multiple_of(AES_BLOCK_BYTES)
        } else {
            SHA1_BYTES
        };
        result.verifier_hash[..hash_bytes].copy_from_slice(take(&mut bytes, hash_bytes)?);
    } else {
        return Err(InspectError::UnsupportedEncryption);
    }
    if !bytes.is_empty() {
        return Err(InspectError::MalformedEncryption);
    }
    Ok(result)
}

fn take<'a>(bytes: &mut &'a [u8], length: usize) -> Result<&'a [u8], InspectError> {
    let (value, rest) = bytes
        .split_at_checked(length)
        .ok_or(InspectError::MalformedEncryption)?;
    *bytes = rest;
    Ok(value)
}
fn word(bytes: &mut &[u8]) -> Result<u32, InspectError> {
    Ok(u32::from_le_bytes(
        take(bytes, size_of::<u32>())?.try_into().unwrap(),
    ))
}
