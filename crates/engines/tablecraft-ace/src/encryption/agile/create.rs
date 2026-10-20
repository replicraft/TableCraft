//! Modern ACCDB profile creation using shared verified password/page primitives.
use super::{
    AES_BLOCK_BYTES, AgileHashAlgorithm, AgilePageDecoder, CipherParameters, MAX_KEY_BYTES, crypto,
    rewrite,
};
use crate::{InspectError, header::check};
use quick_xml::{
    Writer,
    events::{BytesStart, Event},
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use zeroize::Zeroizing;

const KEY_BITS: u16 = 256;
const SPIN_COUNT: u32 = 100_000;
const ENCRYPTION_NS: &str = "http://schemas.microsoft.com/office/2006/encryption";
const PASSWORD_NS: &str = "http://schemas.microsoft.com/office/2006/keyEncryptor/password";
const AGILE_INFO_PREFIX: [u8; 8] = [4, 0, 4, 0, 0x40, 0, 0, 0];

pub(crate) fn create(
    page: &mut [u8],
    password: &str,
    encoding_key: [u8; 4],
    page_count: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<AgilePageDecoder, InspectError> {
    let mut key = Zeroizing::new([0; MAX_KEY_BYTES]);
    let mut params = CipherParameters {
        key_bits: KEY_BITS,
        hash: AgileHashAlgorithm::Sha512,
        salt: [0; AES_BLOCK_BYTES],
    };
    getrandom::fill(&mut *key).map_err(|_| InspectError::EntropyUnavailable)?;
    getrandom::fill(&mut params.salt).map_err(|_| InspectError::EntropyUnavailable)?;
    let fields = rewrite::password_fields(
        (KEY_BITS, params.hash, KEY_BITS),
        SPIN_COUNT,
        password,
        &key,
        budget,
        cancelled,
    )?;
    let salt = rewrite::encode(&params.salt)?;
    let capacity = super::super::OFFICE_INFO_REGION_END;
    budget.charge(Resource::Bytes, capacity as u64)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;
    let mut writer = Writer::new(bytes);
    let mut root = BytesStart::new("encryption");
    root.extend_attributes([("xmlns", ENCRYPTION_NS), ("xmlns:p", PASSWORD_NS)]);
    writer
        .write_event(Event::Start(root))
        .map_err(|_| InspectError::Allocation)?;
    let mut data = BytesStart::new("keyData");
    common(&mut data);
    data.push_attribute(("saltValue", salt.as_str()));
    writer
        .write_event(Event::Empty(data))
        .map_err(|_| InspectError::Allocation)?;
    writer
        .write_event(Event::Start(BytesStart::new("keyEncryptors")))
        .map_err(|_| InspectError::Allocation)?;
    let mut encryptor = BytesStart::new("keyEncryptor");
    encryptor.push_attribute(("uri", PASSWORD_NS));
    writer
        .write_event(Event::Start(encryptor))
        .map_err(|_| InspectError::Allocation)?;
    let mut password_key = BytesStart::new("p:encryptedKey");
    common(&mut password_key);
    let spins = SPIN_COUNT.to_string();
    password_key.push_attribute(("spinCount", spins.as_str()));
    for (name, value) in &fields {
        password_key.push_attribute((*name, value.as_str()));
    }
    writer
        .write_event(Event::Empty(password_key))
        .map_err(|_| InspectError::Allocation)?;
    for name in ["keyEncryptor", "keyEncryptors", "encryption"] {
        writer
            .write_event(Event::End(quick_xml::events::BytesEnd::new(name)))
            .map_err(|_| InspectError::Allocation)?;
    }
    let xml = writer.into_inner();
    let length = AGILE_INFO_PREFIX.len() + xml.len();
    let start = super::super::OFFICE_INFO_OFFSET;
    if start + length > capacity {
        return Err(InspectError::StructureLimit);
    }
    check(budget, cancelled)?;
    page[super::super::OFFICE_INFO_LENGTH_OFFSET..capacity].fill(0);
    page[super::super::OFFICE_INFO_LENGTH_OFFSET..start]
        .copy_from_slice(&(length as u16).to_le_bytes());
    page[start..start + AGILE_INFO_PREFIX.len()].copy_from_slice(&AGILE_INFO_PREFIX);
    page[start + AGILE_INFO_PREFIX.len()..start + length].copy_from_slice(&xml);
    Ok(crypto::encoder(key, params, encoding_key, page_count))
}

fn common(element: &mut BytesStart<'_>) {
    element.extend_attributes([
        ("cipherAlgorithm", "AES"),
        ("cipherChaining", "ChainingModeCBC"),
        ("hashAlgorithm", "SHA512"),
    ]);
    for (name, value) in [
        ("saltSize", AES_BLOCK_BYTES.to_string()),
        ("blockSize", AES_BLOCK_BYTES.to_string()),
        ("keyBits", KEY_BITS.to_string()),
        ("hashSize", AgileHashAlgorithm::Sha512.bytes().to_string()),
    ] {
        element.push_attribute((name, value.as_str()));
    }
}
