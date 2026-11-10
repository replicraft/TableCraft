//! Closed binding for a Windows flat sidecar's immutable metadata prefix.
//! This codec establishes framing and equality, not filesystem authority. Hosts
//! must obtain identities from retained handles and qualify their actual roles.
use super::filesystem_contracts::BindingError;
use crate::{IoError, journal::storage::BLOCK_BYTES, progress::check};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(crate) const HEADER_BYTES: usize = 224;
const MAGIC: &[u8; 8] = b"TCJWIN01";
const PROTOCOL_OFFSET: usize = MAGIC.len();
const SCHEME_OFFSET: usize = PROTOCOL_OFFSET + size_of::<u32>();
const NONCE_OFFSET: usize = SCHEME_OFFSET + size_of::<u32>();
const NONCE_BYTES: usize = 16;
const IDENTITIES_OFFSET: usize = NONCE_OFFSET + NONCE_BYTES;
const FILE_ID_BYTES: usize = 16;
const IDENTITY_BYTES: usize = size_of::<u64>() + FILE_ID_BYTES;
const ROLE_COUNT: usize = 4;
const INTENT_LENGTH_OFFSET: usize = IDENTITIES_OFFSET + ROLE_COUNT * IDENTITY_BYTES;
const JOURNAL_OFFSET_OFFSET: usize = INTENT_LENGTH_OFFSET + size_of::<u64>();
const INTENT_DIGEST_OFFSET: usize = JOURNAL_OFFSET_OFFSET + size_of::<u64>();
const DIGEST_BYTES: usize = 32;
const RESERVED_OFFSET: usize = INTENT_DIGEST_OFFSET + DIGEST_BYTES;
const CHECKSUM_OFFSET: usize = HEADER_BYTES - DIGEST_BYTES;
const PROTOCOL: u32 = 1;
const WINDOWS_IDENTITY_SCHEME: u32 = 2;

/// The complete physical identifier reported by a retained Windows handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FlatIdentity {
    pub(crate) volume: u64,
    pub(crate) file_id: [u8; FILE_ID_BYTES],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FlatObjects {
    pub(crate) target: FlatIdentity,
    pub(crate) target_parent: FlatIdentity,
    pub(crate) sidecar: FlatIdentity,
    pub(crate) sidecar_parent: FlatIdentity,
}

impl FlatObjects {
    fn ordered(self) -> [FlatIdentity; ROLE_COUNT] {
        [
            self.target,
            self.target_parent,
            self.sidecar,
            self.sidecar_parent,
        ]
    }

    fn validate(self) -> Result<(), BindingError> {
        if self.target == self.sidecar
            || [self.target_parent, self.sidecar_parent].contains(&self.target)
            || [self.target_parent, self.sidecar_parent].contains(&self.sidecar)
        {
            return Err(BindingError::AliasedRoles);
        }
        // Both files may intentionally live under the same captured parent.
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FlatBindingData {
    pub(crate) nonce: [u8; NONCE_BYTES],
    pub(crate) objects: FlatObjects,
    pub(crate) intent_length: u64,
    pub(crate) intent_digest: [u8; DIGEST_BYTES],
}

impl FlatBindingData {
    /// Capture an intent digest without allocating or copying the intent.
    pub(crate) fn capture(
        nonce: [u8; NONCE_BYTES],
        objects: FlatObjects,
        intent: &[u8],
        maximum_intent_bytes: u64,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, BindingError> {
        check(budget, cancelled)?;
        objects.validate()?;
        let intent_length = intent.len() as u64;
        checked_offset(intent_length, maximum_intent_bytes)?;
        let intent_digest = digest_intent(intent, budget, cancelled)?;
        Ok(Self {
            nonce,
            objects,
            intent_length,
            intent_digest,
        })
    }

    /// No padding or holes are permitted between this header and its journal.
    pub(crate) fn journal_offset(&self) -> Result<u64, BindingError> {
        (HEADER_BYTES as u64)
            .checked_add(self.intent_length)
            .ok_or(BindingError::InvalidOffset)
    }

    /// Remaining representable physical extent after the immutable prefix.
    /// This is stream geometry, not an image protocol or native file-size limit.
    pub(crate) fn journal_capacity(&self) -> Result<u64, BindingError> {
        Ok(u64::MAX - self.journal_offset()?)
    }

    pub(crate) fn verify_objects(
        &self,
        actual: &FlatObjects,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), BindingError> {
        check(budget, cancelled)?;
        self.objects.validate()?;
        actual.validate()?;
        budget
            .charge(Resource::Steps, (ROLE_COUNT * IDENTITY_BYTES) as u64)
            .map_err(IoError::from)?;
        if self.objects != *actual {
            return Err(BindingError::ObjectMismatch);
        }
        check(budget, cancelled)?;
        Ok(())
    }

    pub(crate) fn verify_intent(
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

    pub(crate) fn encode(
        &self,
        maximum_intent_bytes: u64,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<[u8; HEADER_BYTES], BindingError> {
        check(budget, cancelled)?;
        self.objects.validate()?;
        let journal_offset = checked_offset(self.intent_length, maximum_intent_bytes)?;
        budget
            .charge(Resource::Steps, HEADER_BYTES as u64)
            .map_err(IoError::from)?;
        let mut bytes = [0; HEADER_BYTES];
        bytes[..MAGIC.len()].copy_from_slice(MAGIC);
        bytes[PROTOCOL_OFFSET..SCHEME_OFFSET].copy_from_slice(&PROTOCOL.to_le_bytes());
        bytes[SCHEME_OFFSET..NONCE_OFFSET].copy_from_slice(&WINDOWS_IDENTITY_SCHEME.to_le_bytes());
        bytes[NONCE_OFFSET..IDENTITIES_OFFSET].copy_from_slice(&self.nonce);
        for (position, identity) in self.objects.ordered().iter().enumerate() {
            let start = IDENTITIES_OFFSET + position * IDENTITY_BYTES;
            let split = start + size_of::<u64>();
            bytes[start..split].copy_from_slice(&identity.volume.to_le_bytes());
            bytes[split..start + IDENTITY_BYTES].copy_from_slice(&identity.file_id);
        }
        bytes[INTENT_LENGTH_OFFSET..JOURNAL_OFFSET_OFFSET]
            .copy_from_slice(&self.intent_length.to_le_bytes());
        bytes[JOURNAL_OFFSET_OFFSET..INTENT_DIGEST_OFFSET]
            .copy_from_slice(&journal_offset.to_le_bytes());
        bytes[INTENT_DIGEST_OFFSET..RESERVED_OFFSET].copy_from_slice(&self.intent_digest);
        let checksum = Sha256::digest(&bytes[..CHECKSUM_OFFSET]);
        bytes[CHECKSUM_OFFSET..].copy_from_slice(&checksum);
        check(budget, cancelled)?;
        Ok(bytes)
    }
}

pub(crate) fn decode(
    bytes: &[u8],
    maximum_intent_bytes: u64,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<FlatBindingData, BindingError> {
    check(budget, cancelled)?;
    if bytes.len() != HEADER_BYTES {
        return Err(BindingError::InvalidLength);
    }
    budget
        .charge(Resource::Steps, HEADER_BYTES as u64)
        .map_err(IoError::from)?;
    if &bytes[..MAGIC.len()] != MAGIC {
        return Err(BindingError::InvalidMagic);
    }
    if u32::from_le_bytes(
        bytes[PROTOCOL_OFFSET..SCHEME_OFFSET]
            .try_into()
            .expect("fixed header"),
    ) != PROTOCOL
    {
        return Err(BindingError::UnsupportedProtocol);
    }
    if u32::from_le_bytes(
        bytes[SCHEME_OFFSET..NONCE_OFFSET]
            .try_into()
            .expect("fixed header"),
    ) != WINDOWS_IDENTITY_SCHEME
    {
        return Err(BindingError::UnsupportedIdentityScheme);
    }
    if bytes[RESERVED_OFFSET..CHECKSUM_OFFSET]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(BindingError::InvalidFlags);
    }
    let checksum = Sha256::digest(&bytes[..CHECKSUM_OFFSET]);
    if bytes[CHECKSUM_OFFSET..] != checksum[..] {
        return Err(BindingError::ChecksumMismatch);
    }

    let intent_length = u64::from_le_bytes(
        bytes[INTENT_LENGTH_OFFSET..JOURNAL_OFFSET_OFFSET]
            .try_into()
            .expect("fixed header"),
    );
    let expected_offset = checked_offset(intent_length, maximum_intent_bytes)?;
    let offset = u64::from_le_bytes(
        bytes[JOURNAL_OFFSET_OFFSET..INTENT_DIGEST_OFFSET]
            .try_into()
            .expect("fixed header"),
    );
    if offset != expected_offset {
        return Err(BindingError::InvalidOffset);
    }
    let identities = std::array::from_fn::<_, ROLE_COUNT, _>(|position| {
        let start = IDENTITIES_OFFSET + position * IDENTITY_BYTES;
        let split = start + size_of::<u64>();
        FlatIdentity {
            volume: u64::from_le_bytes(bytes[start..split].try_into().expect("fixed identity")),
            file_id: bytes[split..start + IDENTITY_BYTES]
                .try_into()
                .expect("fixed identity"),
        }
    });
    let objects = FlatObjects {
        target: identities[0],
        target_parent: identities[1],
        sidecar: identities[2],
        sidecar_parent: identities[3],
    };
    objects.validate()?;
    let data = FlatBindingData {
        nonce: bytes[NONCE_OFFSET..IDENTITIES_OFFSET]
            .try_into()
            .expect("fixed header"),
        objects,
        intent_length,
        intent_digest: bytes[INTENT_DIGEST_OFFSET..RESERVED_OFFSET]
            .try_into()
            .expect("fixed header"),
    };
    check(budget, cancelled)?;
    Ok(data)
}

fn checked_offset(length: u64, maximum: u64) -> Result<u64, BindingError> {
    if length > maximum {
        return Err(BindingError::IntentLimit);
    }
    (HEADER_BYTES as u64)
        .checked_add(length)
        .ok_or(BindingError::InvalidOffset)
}

fn digest_intent(
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
    check(budget, cancelled)?;
    Ok(hash.finalize().into())
}

#[cfg(test)]
mod tests;
