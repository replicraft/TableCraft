use crate::{IoError, journal::storage::BLOCK_BYTES, namespace::ObjectIdentity, progress::check};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::{Resource, ResourceBudget};
use thiserror::Error;

pub(super) const RECORD_BYTES: usize = 184;
const MAGIC: &[u8; 8] = b"TCJPAIR1";
const PROTOCOL_OFFSET: usize = MAGIC.len();
const SCHEME_OFFSET: usize = PROTOCOL_OFFSET + size_of::<u32>();
const NONCE_OFFSET: usize = SCHEME_OFFSET + size_of::<u32>();
const NONCE_BYTES: usize = 16;
const IDENTITIES_OFFSET: usize = NONCE_OFFSET + NONCE_BYTES;
const IDENTITY_BYTES: usize = 2 * size_of::<u64>();
const ROLE_COUNT: usize = 5;
const INTENT_LENGTH_OFFSET: usize = IDENTITIES_OFFSET + ROLE_COUNT * IDENTITY_BYTES;
const INTENT_DIGEST_OFFSET: usize = INTENT_LENGTH_OFFSET + size_of::<u64>();
const DIGEST_BYTES: usize = 32;
const CHECKSUM_OFFSET: usize = INTENT_DIGEST_OFFSET + DIGEST_BYTES;
const FIXED_V1_PROTOCOL: u32 = 1;
const UNIX_OBJECT_SCHEME: u32 = 1;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BindingError {
    #[error(transparent)]
    Io(#[from] IoError),
    #[error("invalid pair binding record length")]
    InvalidLength,
    #[error("invalid pair binding record signature")]
    InvalidMagic,
    #[error("unsupported paired journal protocol")]
    UnsupportedProtocol,
    #[error("unsupported pair object identity scheme")]
    UnsupportedIdentityScheme,
    #[error("pair binding checksum mismatch")]
    ChecksumMismatch,
    #[error("pair storage roles alias the same object")]
    AliasedRoles,
    #[error("pair binding does not match actual retained objects")]
    ObjectMismatch,
    #[error("paired intent length mismatch")]
    IntentLengthMismatch,
    #[error("paired intent digest mismatch")]
    IntentDigestMismatch,
}

#[derive(Clone, Copy)]
pub(super) struct PairObjects {
    pub target: ObjectIdentity,
    pub bundle: ObjectIdentity,
    pub binding: ObjectIdentity,
    pub intent: ObjectIdentity,
    pub journal: ObjectIdentity,
}

impl PairObjects {
    fn ordered(self) -> [ObjectIdentity; ROLE_COUNT] {
        [
            self.target,
            self.bundle,
            self.binding,
            self.intent,
            self.journal,
        ]
    }

    fn distinct(self) -> Result<(), BindingError> {
        let roles = self.ordered();
        for (position, identity) in roles.iter().enumerate() {
            if roles[..position].contains(identity) {
                return Err(BindingError::AliasedRoles);
            }
        }
        Ok(())
    }
}

pub(super) struct BindingData {
    pub nonce: [u8; NONCE_BYTES],
    pub target: ObjectIdentity,
    pub bundle: ObjectIdentity,
    pub binding: ObjectIdentity,
    pub intent: ObjectIdentity,
    pub journal: ObjectIdentity,
    pub intent_length: u64,
    pub intent_digest: [u8; DIGEST_BYTES],
}

impl BindingData {
    fn objects(&self) -> PairObjects {
        PairObjects {
            target: self.target,
            bundle: self.bundle,
            binding: self.binding,
            intent: self.intent,
            journal: self.journal,
        }
    }

    pub(super) fn verify_objects(&self, actual: &PairObjects) -> Result<(), BindingError> {
        actual.distinct()?;
        if self.objects().ordered() != actual.ordered() {
            return Err(BindingError::ObjectMismatch);
        }
        Ok(())
    }

    pub(super) fn verify_intent(
        &self,
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), BindingError> {
        check(budget, cancelled)?;
        if bytes.len() as u64 != self.intent_length {
            return Err(BindingError::IntentLengthMismatch);
        }
        if digest_intent(bytes, budget, cancelled)? != self.intent_digest {
            return Err(BindingError::IntentDigestMismatch);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn encode(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<[u8; RECORD_BYTES], BindingError> {
        check(budget, cancelled)?;
        self.objects().distinct()?;
        budget
            .charge(Resource::Steps, RECORD_BYTES as u64)
            .map_err(IoError::from)?;
        let mut bytes = [0; RECORD_BYTES];
        bytes[..MAGIC.len()].copy_from_slice(MAGIC);
        bytes[PROTOCOL_OFFSET..SCHEME_OFFSET].copy_from_slice(&FIXED_V1_PROTOCOL.to_le_bytes());
        bytes[SCHEME_OFFSET..NONCE_OFFSET].copy_from_slice(&UNIX_OBJECT_SCHEME.to_le_bytes());
        bytes[NONCE_OFFSET..IDENTITIES_OFFSET].copy_from_slice(&self.nonce);
        for (position, identity) in self.objects().ordered().iter().enumerate() {
            let start = IDENTITIES_OFFSET + position * IDENTITY_BYTES;
            bytes[start..start + size_of::<u64>()].copy_from_slice(&identity.device.to_le_bytes());
            bytes[start + size_of::<u64>()..start + IDENTITY_BYTES]
                .copy_from_slice(&identity.inode.to_le_bytes());
        }
        bytes[INTENT_LENGTH_OFFSET..INTENT_DIGEST_OFFSET]
            .copy_from_slice(&self.intent_length.to_le_bytes());
        bytes[INTENT_DIGEST_OFFSET..CHECKSUM_OFFSET].copy_from_slice(&self.intent_digest);
        let checksum = Sha256::digest(&bytes[..CHECKSUM_OFFSET]);
        bytes[CHECKSUM_OFFSET..].copy_from_slice(&checksum);
        check(budget, cancelled)?;
        Ok(bytes)
    }
}

pub(super) fn decode(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<BindingData, BindingError> {
    check(budget, cancelled)?;
    if bytes.len() != RECORD_BYTES {
        return Err(BindingError::InvalidLength);
    }
    budget
        .charge(Resource::Steps, RECORD_BYTES as u64)
        .map_err(IoError::from)?;
    if &bytes[..MAGIC.len()] != MAGIC {
        return Err(BindingError::InvalidMagic);
    }
    if u32::from_le_bytes(
        bytes[PROTOCOL_OFFSET..SCHEME_OFFSET]
            .try_into()
            .expect("fixed record"),
    ) != FIXED_V1_PROTOCOL
    {
        return Err(BindingError::UnsupportedProtocol);
    }
    if u32::from_le_bytes(
        bytes[SCHEME_OFFSET..NONCE_OFFSET]
            .try_into()
            .expect("fixed record"),
    ) != UNIX_OBJECT_SCHEME
    {
        return Err(BindingError::UnsupportedIdentityScheme);
    }
    let checksum: [u8; DIGEST_BYTES] = Sha256::digest(&bytes[..CHECKSUM_OFFSET]).into();
    if checksum != bytes[CHECKSUM_OFFSET..] {
        return Err(BindingError::ChecksumMismatch);
    }
    let mut identities = [ObjectIdentity {
        device: 0,
        inode: 0,
    }; ROLE_COUNT];
    for (position, identity) in identities.iter_mut().enumerate() {
        let start = IDENTITIES_OFFSET + position * IDENTITY_BYTES;
        identity.device = u64::from_le_bytes(
            bytes[start..start + size_of::<u64>()]
                .try_into()
                .expect("fixed record"),
        );
        identity.inode = u64::from_le_bytes(
            bytes[start + size_of::<u64>()..start + IDENTITY_BYTES]
                .try_into()
                .expect("fixed record"),
        );
    }
    let data = BindingData {
        nonce: bytes[NONCE_OFFSET..IDENTITIES_OFFSET]
            .try_into()
            .expect("fixed record"),
        target: identities[0],
        bundle: identities[1],
        binding: identities[2],
        intent: identities[3],
        journal: identities[4],
        intent_length: u64::from_le_bytes(
            bytes[INTENT_LENGTH_OFFSET..INTENT_DIGEST_OFFSET]
                .try_into()
                .expect("fixed record"),
        ),
        intent_digest: bytes[INTENT_DIGEST_OFFSET..CHECKSUM_OFFSET]
            .try_into()
            .expect("fixed record"),
    };
    data.objects().distinct()?;
    check(budget, cancelled)?;
    Ok(data)
}

pub(super) fn digest_intent(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<[u8; DIGEST_BYTES], BindingError> {
    check(budget, cancelled)?;
    let mut hash = Sha256::new();
    for chunk in bytes.chunks(BLOCK_BYTES) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, chunk.len() as u64)
            .map_err(IoError::from)?;
        hash.update(chunk);
    }
    Ok(hash.finalize().into())
}

#[cfg(test)]
mod tests;
