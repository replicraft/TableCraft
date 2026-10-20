use super::{
    AES_BLOCK_BYTES, AgileHashAlgorithm, CipherParameters, Descriptor, MAX_HASH_BYTES,
    MAX_KEY_BYTES, MAX_SPIN_COUNT,
};
use crate::{InspectError, header::check};
use base64::{Engine, engine::general_purpose::STANDARD};
use quick_xml::{
    NsReader,
    events::{BytesStart, Event},
    name::ResolveResult,
};
use std::borrow::Cow;
use tablecraft_core::limits::{Resource, ResourceBudget};

const ENCRYPTION_NS: &str = "http://schemas.microsoft.com/office/2006/encryption";
const PASSWORD_NS: &str = "http://schemas.microsoft.com/office/2006/keyEncryptor/password";
const MAX_DEPTH: usize = 4;
const MAX_ATTRIBUTES: usize = 15;
const BASE64_BUFFER_BYTES: usize = 128;
const FIELDS: [&str; MAX_ATTRIBUTES] = [
    "saltSize",
    "blockSize",
    "keyBits",
    "hashSize",
    "cipherAlgorithm",
    "cipherChaining",
    "hashAlgorithm",
    "saltValue",
    "spinCount",
    "encryptedVerifierHashInput",
    "encryptedVerifierHashValue",
    "encryptedKeyValue",
    "uri",
    "encryptedHmacKey",
    "encryptedHmacValue",
];
const SALT_SIZE: usize = 0;
const BLOCK_SIZE: usize = 1;
const KEY_BITS: usize = 2;
const HASH_SIZE: usize = 3;
const CIPHER: usize = 4;
const CHAINING: usize = 5;
const HASH: usize = 6;
const SALT: usize = 7;
const SPINS: usize = 8;
const VERIFIER: usize = 9;
const VERIFIER_HASH: usize = 10;
const KEY: usize = 11;
const URI: usize = 12;
const HMAC_KEY: usize = 13;
const HMAC_VALUE: usize = 14;
const COMMON_FIELDS: u16 = (1 << SPINS) - 1;
const PASSWORD_FIELDS: u16 = (1 << URI) - 1;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Node {
    Root,
    KeyData,
    Integrity,
    Encryptors,
    Encryptor,
    Password,
}

struct Attributes<'a>([Option<Cow<'a, str>>; MAX_ATTRIBUTES]);
impl<'a> Attributes<'a> {
    fn read(
        e: &'a BytesStart<'a>,
        r: &NsReader<&[u8]>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        let mut fields = Self(std::array::from_fn(|_| None));
        for attribute in e.attributes() {
            check(budget, cancelled)?;
            let attribute = attribute.map_err(|_| InspectError::MalformedEncryption)?;
            if attribute.key.as_ref() == "xmlns" || attribute.key.as_ref().starts_with("xmlns:") {
                continue;
            }
            let (namespace, local) = r.resolver().resolve_attribute(attribute.key);
            if !matches!(namespace, ResolveResult::Unbound) {
                return Err(InspectError::UnsupportedEncryption);
            }
            let field = FIELDS
                .iter()
                .position(|f| *f == local.as_ref())
                .ok_or(InspectError::UnsupportedEncryption)?;
            if fields.0[field].is_some() {
                return Err(InspectError::MalformedEncryption);
            }
            fields.0[field] = Some(
                attribute
                    .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                    .map_err(|_| InspectError::MalformedEncryption)?,
            );
        }
        Ok(fields)
    }
    fn only(&self, fields: u16) -> Result<(), InspectError> {
        if self
            .0
            .iter()
            .enumerate()
            .any(|(i, v)| v.is_some() && fields & (1 << i) == 0)
        {
            return Err(InspectError::UnsupportedEncryption);
        }
        Ok(())
    }
    fn get(&self, field: usize) -> Result<&str, InspectError> {
        self.0[field]
            .as_deref()
            .ok_or(InspectError::MalformedEncryption)
    }
    fn number(&self, field: usize) -> Result<u32, InspectError> {
        let value = self
            .get(field)?
            .trim_matches(|c| matches!(c, ' ' | '\t' | '\r' | '\n'));
        if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
            return Err(InspectError::MalformedEncryption);
        }
        value.parse().map_err(|_| InspectError::MalformedEncryption)
    }
    fn decoded<const N: usize>(
        &self,
        field: usize,
        expected: usize,
    ) -> Result<[u8; N], InspectError> {
        let (out, decoded) = self.decoded_any(field)?;
        if decoded != expected {
            return Err(InspectError::MalformedEncryption);
        }
        Ok(out)
    }
    fn decoded_any<const N: usize>(&self, field: usize) -> Result<([u8; N], usize), InspectError> {
        let mut encoded = [0; BASE64_BUFFER_BYTES];
        let mut length = 0;
        for byte in self.get(field)?.bytes() {
            if matches!(byte, b' ' | b'\t' | b'\r' | b'\n') {
                continue;
            }
            let slot = encoded
                .get_mut(length)
                .ok_or(InspectError::MalformedEncryption)?;
            *slot = byte;
            length += 1;
        }
        let mut out = [0; N];
        let decoded = STANDARD
            .decode_slice(&encoded[..length], &mut out)
            .map_err(|_| InspectError::MalformedEncryption)?;
        Ok((out, decoded))
    }
    fn cipher(&self) -> Result<CipherParameters, InspectError> {
        if self.get(CIPHER)? != "AES" || self.get(CHAINING)? != "ChainingModeCBC" {
            return Err(InspectError::UnsupportedEncryption);
        }
        if self.number(SALT_SIZE)? != AES_BLOCK_BYTES as u32
            || self.number(BLOCK_SIZE)? != AES_BLOCK_BYTES as u32
        {
            return Err(InspectError::UnsupportedEncryption);
        }
        let key_bits = match self.number(KEY_BITS)? {
            n @ (128 | 192 | 256) => n as u16,
            _ => return Err(InspectError::UnsupportedEncryption),
        };
        let hash = match self.get(HASH)? {
            "SHA1" | "SHA-1" => AgileHashAlgorithm::Sha1,
            "SHA256" | "SHA-256" => AgileHashAlgorithm::Sha256,
            "SHA384" | "SHA-384" => AgileHashAlgorithm::Sha384,
            "SHA512" | "SHA-512" => AgileHashAlgorithm::Sha512,
            _ => return Err(InspectError::UnsupportedEncryption),
        };
        if self.number(HASH_SIZE)? != hash.bytes() as u32 {
            return Err(InspectError::MalformedEncryption);
        }
        Ok(CipherParameters {
            key_bits,
            hash,
            salt: self.decoded(SALT, AES_BLOCK_BYTES)?,
        })
    }
}

pub(super) fn parse(
    xml: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Descriptor, InspectError> {
    // The enclosing native descriptor already bounds the XML to one 4 KiB page.
    // Account conservatively for bounded parser/name/unescape allocations up front.
    budget.charge(Resource::Bytes, (xml.len() as u64) * 8)?;
    std::str::from_utf8(xml).map_err(|_| InspectError::MalformedEncryption)?;
    let mut reader = NsReader::from_reader(xml);
    reader.config_mut().check_comments = true;
    let mut stack = [Node::Root; MAX_DEPTH];
    let mut depth = 0;
    let mut root_seen = false;
    let mut declaration_seen = false;
    let mut data = None;
    let mut password = None;
    let mut encrypted_values = None;
    let mut spins = None;
    let mut integrity = false;
    let mut encryptors_seen = false;
    let mut encryptor_seen = false;
    loop {
        check(budget, cancelled)?;
        let event = reader
            .read_event()
            .map_err(|_| InspectError::MalformedEncryption)?;
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                if depth == MAX_DEPTH {
                    return Err(InspectError::UnsupportedEncryption);
                }
                let (ns, local) = reader.resolver().resolve_element(e.name());
                let ns = match ns {
                    ResolveResult::Bound(ns) => ns,
                    _ => return Err(InspectError::MalformedEncryption),
                };
                let parent = depth.checked_sub(1).map(|i| stack[i]);
                let node = match (parent, ns.as_ref(), local.as_ref()) {
                    (None, ENCRYPTION_NS, "encryption") if !root_seen => {
                        root_seen = true;
                        Node::Root
                    }
                    (Some(Node::Root), ENCRYPTION_NS, "keyData") if data.is_none() => Node::KeyData,
                    (Some(Node::Root), ENCRYPTION_NS, "dataIntegrity") if !integrity => {
                        Node::Integrity
                    }
                    (Some(Node::Root), ENCRYPTION_NS, "keyEncryptors") if !encryptors_seen => {
                        encryptors_seen = true;
                        Node::Encryptors
                    }
                    (Some(Node::Encryptors), ENCRYPTION_NS, "keyEncryptor") if !encryptor_seen => {
                        encryptor_seen = true;
                        Node::Encryptor
                    }
                    (Some(Node::Encryptor), PASSWORD_NS, "encryptedKey") if password.is_none() => {
                        Node::Password
                    }
                    _ => return Err(InspectError::UnsupportedEncryption),
                };
                let a = Attributes::read(e, &reader, budget, cancelled)?;
                match node {
                    Node::Root | Node::Encryptors => a.only(0)?,
                    Node::KeyData => {
                        a.only(COMMON_FIELDS)?;
                        data = Some(a.cipher()?);
                    }
                    Node::Encryptor => {
                        a.only(1 << URI)?;
                        if a.get(URI)? != PASSWORD_NS {
                            return Err(InspectError::UnsupportedEncryption);
                        }
                    }
                    Node::Password => {
                        a.only(PASSWORD_FIELDS)?;
                        let p = a.cipher()?;
                        let count = a.number(SPINS)?;
                        if count > MAX_SPIN_COUNT {
                            return Err(InspectError::MalformedEncryption);
                        }
                        let verifier = a.decoded::<AES_BLOCK_BYTES>(VERIFIER, AES_BLOCK_BYTES)?;
                        let hash_bytes = p.hash.bytes().next_multiple_of(AES_BLOCK_BYTES);
                        let verifier_hash =
                            a.decoded::<MAX_HASH_BYTES>(VERIFIER_HASH, hash_bytes)?;
                        // Length depends on keyData; validate after the entire document.
                        let (key, key_len) = a.decoded_any::<MAX_KEY_BYTES>(KEY)?;
                        if key_len == 0 || !key_len.is_multiple_of(AES_BLOCK_BYTES) {
                            return Err(InspectError::MalformedEncryption);
                        }
                        encrypted_values = Some((verifier, verifier_hash, key, key_len));
                        password = Some(p);
                        spins = Some(count);
                    }
                    Node::Integrity => {
                        a.only((1 << HMAC_KEY) | (1 << HMAC_VALUE))?;
                        // These fields are shape-checked only; this page codec does not verify file HMACs.
                        for f in [HMAC_KEY, HMAC_VALUE] {
                            let (_, n) = a.decoded_any::<MAX_HASH_BYTES>(f)?;
                            if n == 0 || !n.is_multiple_of(AES_BLOCK_BYTES) {
                                return Err(InspectError::MalformedEncryption);
                            }
                        }
                        integrity = true;
                    }
                }
                if matches!(event, Event::Start(_)) {
                    stack[depth] = node;
                    depth += 1;
                }
            }
            Event::End(_) => {
                depth = depth
                    .checked_sub(1)
                    .ok_or(InspectError::MalformedEncryption)?;
            }
            Event::Text(t)
                if t.as_ref()
                    .bytes()
                    .all(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n')) => {}
            Event::Comment(_) => {}
            Event::Decl(d) if !root_seen && !declaration_seen => {
                declaration_seen = true;
                if d.version().map_err(|_| InspectError::MalformedEncryption)? != "1.0" {
                    return Err(InspectError::UnsupportedEncryption);
                }
                if let Some(enc) = d.encoding()
                    && !enc
                        .map_err(|_| InspectError::MalformedEncryption)?
                        .eq_ignore_ascii_case("UTF-8")
                {
                    return Err(InspectError::UnsupportedEncryption);
                }
            }
            Event::Eof => break,
            // No DTDs, entity declarations/references, callbacks or external resources.
            _ => return Err(InspectError::UnsupportedEncryption),
        }
    }
    if !root_seen || depth != 0 {
        return Err(InspectError::MalformedEncryption);
    }
    let data = data.ok_or(InspectError::MalformedEncryption)?;
    let password = password.ok_or(InspectError::MalformedEncryption)?;
    let (verifier, verifier_hash, key, key_len) =
        encrypted_values.ok_or(InspectError::MalformedEncryption)?;
    if key_len != (usize::from(data.key_bits) / u8::BITS as usize).next_multiple_of(AES_BLOCK_BYTES)
    {
        return Err(InspectError::MalformedEncryption);
    }
    Ok(Descriptor {
        data,
        password,
        spins: spins.ok_or(InspectError::MalformedEncryption)?,
        verifier,
        verifier_hash,
        key,
        has_integrity: integrity,
    })
}
