//! Rewrap an existing file key without changing data pages or native SID masks.
use super::{AES_BLOCK_BYTES, CipherParameters, MAX_KEY_BYTES, crypto, read_agile_encryption};
use crate::{InspectError, header::check};
use aes::cipher::{BlockModeEncrypt, KeyIvInit, block_padding::NoPadding};
use base64::{Engine, engine::general_purpose::STANDARD};
use quick_xml::{
    Reader, Writer,
    events::{BytesStart, Event},
};
use std::io::{Read, Seek};
use tablecraft_core::limits::{Resource, ResourceBudget};
use zeroize::Zeroizing;

const BASE64_BUFFER_BYTES: usize = 128;
const AGILE_INFO_PREFIX: [u8; 8] = [4, 0, 4, 0, 0x40, 0, 0, 0];

pub(crate) fn change_password<R: Read + Seek>(
    source: &mut R,
    page: &mut [u8],
    old: Option<&str>,
    password: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    let encryption = read_agile_encryption(source, budget, cancelled)?;
    let decoder = encryption.unlock(old, budget, cancelled)?;
    let descriptor = encryption.descriptor;
    let mut params = CipherParameters {
        key_bits: descriptor.password.key_bits,
        hash: descriptor.password.hash,
        salt: [0; AES_BLOCK_BYTES],
    };
    getrandom::fill(&mut params.salt).map_err(|_| InspectError::EntropyUnavailable)?;
    let iterated = crypto::password_hash(&params, password, descriptor.spins, budget, cancelled)?;
    let mut verifier = Zeroizing::new([0; AES_BLOCK_BYTES]);
    getrandom::fill(&mut *verifier).map_err(|_| InspectError::EntropyUnavailable)?;
    let mut hash = crypto::digest(params.hash, &[&*verifier]);
    encrypt(
        &params,
        &crypto::derive(&params, &iterated, &crypto::VERIFIER_INPUT_BLOCK),
        &mut *verifier,
    )?;
    let hash_length = params.hash.bytes().next_multiple_of(AES_BLOCK_BYTES);
    encrypt(
        &params,
        &crypto::derive(&params, &iterated, &crypto::VERIFIER_HASH_BLOCK),
        &mut hash[..hash_length],
    )?;
    let mut key = Zeroizing::new(*decoder.key);
    let key_length = crypto::key_bytes(descriptor.data.key_bits).next_multiple_of(AES_BLOCK_BYTES);
    encrypt(
        &params,
        &crypto::derive(&params, &iterated, &crypto::KEY_VALUE_BLOCK),
        &mut key[..key_length],
    )?;
    // All emitted strings contain only encoded salts/ciphertext, never plaintext keys.
    let replacements = [
        ("saltValue", encode(&params.salt)?),
        ("encryptedVerifierHashInput", encode(&*verifier)?),
        ("encryptedVerifierHashValue", encode(&hash[..hash_length])?),
        ("encryptedKeyValue", encode(&key[..key_length])?),
    ];
    let info = super::super::parse_office(page)?;
    let start = super::super::OFFICE_INFO_OFFSET + super::super::FLAGS_END;
    let end = super::super::OFFICE_INFO_OFFSET + usize::from(info.descriptor_bytes());
    if end > super::super::OFFICE_INFO_REGION_END {
        return Err(InspectError::UnsupportedEncryption);
    }
    budget.charge(
        Resource::Bytes,
        (super::super::OFFICE_INFO_REGION_END * 2) as u64,
    )?;
    let mut reader = Reader::from_reader(&page[start..end]);
    let mut storage = Vec::new();
    storage
        .try_reserve_exact(super::super::OFFICE_INFO_REGION_END * 2)
        .map_err(|_| InspectError::Allocation)?;
    let mut writer = Writer::new(storage);
    let mut changed = 0;
    loop {
        check(budget, cancelled)?;
        let event = reader
            .read_event()
            .map_err(|_| InspectError::MalformedEncryption)?;
        let empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Start(ref element) | Event::Empty(ref element)
                if element.local_name().as_ref() == "encryptedKey" =>
            {
                let mut updated = BytesStart::new(element.name().as_ref().to_owned());
                for attribute in element.attributes() {
                    let attribute = attribute.map_err(|_| InspectError::MalformedEncryption)?;
                    let name = attribute.key.as_ref();
                    let old = attribute
                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .map_err(|_| InspectError::MalformedEncryption)?;
                    let value = replacements
                        .iter()
                        .find(|(key, _)| *key == name)
                        .map_or(old.as_ref(), |(_, value)| value.as_str());
                    updated.push_attribute((name, value));
                }
                writer
                    .write_event(if empty {
                        Event::Empty(updated)
                    } else {
                        Event::Start(updated)
                    })
                    .map_err(|_| InspectError::Allocation)?;
                changed += 1;
            }
            Event::Eof => break,
            event => writer
                .write_event(event)
                .map_err(|_| InspectError::Allocation)?,
        }
    }
    if changed != 1 {
        return Err(InspectError::MalformedEncryption);
    }
    let xml = writer.into_inner();
    let length = super::super::FLAGS_END + xml.len();
    if super::super::OFFICE_INFO_OFFSET + length > super::super::OFFICE_INFO_REGION_END {
        return Err(InspectError::StructureLimit);
    }
    check(budget, cancelled)?;
    page[super::super::OFFICE_INFO_LENGTH_OFFSET..super::super::OFFICE_INFO_REGION_END].fill(0);
    page[super::super::OFFICE_INFO_LENGTH_OFFSET..super::super::OFFICE_INFO_OFFSET]
        .copy_from_slice(&(length as u16).to_le_bytes());
    page[super::super::OFFICE_INFO_OFFSET..start].copy_from_slice(&AGILE_INFO_PREFIX);
    page[start..start + xml.len()].copy_from_slice(&xml);
    Ok(())
}

fn encode(bytes: &[u8]) -> Result<String, InspectError> {
    let mut buffer = [0; BASE64_BUFFER_BYTES];
    let length = STANDARD
        .encode_slice(bytes, &mut buffer)
        .map_err(|_| InspectError::MalformedEncryption)?;
    Ok(std::str::from_utf8(&buffer[..length])
        .map_err(|_| InspectError::MalformedEncryption)?
        .to_owned())
}

fn encrypt(
    params: &CipherParameters,
    key: &[u8; MAX_KEY_BYTES],
    data: &mut [u8],
) -> Result<(), InspectError> {
    macro_rules! encrypt {
        ($cipher:ty) => {{
            let length = data.len();
            cbc::Encryptor::<$cipher>::new_from_slices(
                &key[..crypto::key_bytes(params.key_bits)],
                &params.salt,
            )
            .map_err(|_| InspectError::MalformedEncryption)?
            .encrypt_padded::<NoPadding>(data, length)
            .map_err(|_| InspectError::MalformedEncryption)?;
        }};
    }
    match params.key_bits {
        128 => encrypt!(aes::Aes128),
        192 => encrypt!(aes::Aes192),
        256 => encrypt!(aes::Aes256),
        _ => return Err(InspectError::UnsupportedEncryption),
    }
    Ok(())
}
