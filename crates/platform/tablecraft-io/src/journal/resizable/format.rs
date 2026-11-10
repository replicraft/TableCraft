//! Both lengths are independently bound to the versioned image digest.
use super::super::{
    JournalError,
    format::MAX_IMAGE_BYTES,
    storage::{self, BLOCK_BYTES, DurableStorage},
};
use super::images;
use sha2::{Digest, Sha256};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) const MAGIC: &[u8; 8] = b"TCIMG002";
const COMMITTED: &[u8; 8] = b"COMMIT02";
const BEFORE_LENGTH_OFFSET: usize = MAGIC.len();
const AFTER_LENGTH_OFFSET: usize = BEFORE_LENGTH_OFFSET + size_of::<u64>();
const DIGEST_OFFSET: usize = AFTER_LENGTH_OFFSET + size_of::<u64>();
const DIGEST_BYTES: usize = 32;
pub(super) const HEADER_BYTES: usize = DIGEST_OFFSET + DIGEST_BYTES;
const MARKER_BYTES: usize = COMMITTED.len() + DIGEST_BYTES;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Protocol {
    V2,
    V3,
}

impl Protocol {
    pub fn magic(self) -> &'static [u8; 8] {
        match self {
            Self::V2 => MAGIC,
            Self::V3 => b"TCIMG003",
        }
    }

    fn committed(self) -> &'static [u8; 8] {
        match self {
            Self::V2 => COMMITTED,
            Self::V3 => b"COMMIT03",
        }
    }

    pub fn body_end(self, before: u64, after: u64) -> Result<u64, JournalError> {
        if self == Self::V2 && (before > MAX_IMAGE_BYTES || after > MAX_IMAGE_BYTES) {
            return Err(JournalError::InvalidJournal);
        }
        usize::try_from(before).map_err(|_| JournalError::InvalidJournal)?;
        usize::try_from(after).map_err(|_| JournalError::InvalidJournal)?;
        let end = (HEADER_BYTES as u64)
            .checked_add(before)
            .and_then(|size| size.checked_add(after))
            .ok_or(JournalError::InvalidJournal)?;
        end.checked_add(MARKER_BYTES as u64)
            .ok_or(JournalError::InvalidJournal)?;
        Ok(end)
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum State {
    Pending,
    Committed,
}

pub(super) struct Record {
    pub protocol: Protocol,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub digest: [u8; DIGEST_BYTES],
    pub state: State,
}

impl Record {
    pub fn body_end(&self) -> u64 {
        HEADER_BYTES as u64 + self.before.len() as u64 + self.after.len() as u64
    }

    pub fn header(&self) -> [u8; HEADER_BYTES] {
        let mut bytes = [0; HEADER_BYTES];
        bytes[..BEFORE_LENGTH_OFFSET].copy_from_slice(self.protocol.magic());
        bytes[BEFORE_LENGTH_OFFSET..AFTER_LENGTH_OFFSET]
            .copy_from_slice(&(self.before.len() as u64).to_le_bytes());
        bytes[AFTER_LENGTH_OFFSET..DIGEST_OFFSET]
            .copy_from_slice(&(self.after.len() as u64).to_le_bytes());
        bytes[DIGEST_OFFSET..].copy_from_slice(&self.digest);
        bytes
    }

    pub fn marker(&self) -> [u8; MARKER_BYTES] {
        let mut bytes = [0; MARKER_BYTES];
        bytes[..COMMITTED.len()].copy_from_slice(self.protocol.committed());
        bytes[COMMITTED.len()..].copy_from_slice(&self.digest);
        bytes
    }
}

pub(super) fn digest(
    protocol: Protocol,
    before: &[u8],
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<[u8; DIGEST_BYTES], JournalError> {
    let mut hash = Sha256::new();
    hash.update(protocol.magic());
    hash.update((before.len() as u64).to_le_bytes());
    hash.update((after.len() as u64).to_le_bytes());
    for image in [before, after] {
        for chunk in image.chunks(BLOCK_BYTES) {
            storage::check(budget, cancelled)?;
            budget.charge(Resource::Steps, chunk.len() as u64)?;
            hash.update(chunk);
        }
    }
    storage::check(budget, cancelled)?;
    Ok(hash.finalize().into())
}

pub(super) fn load<J: DurableStorage>(
    journal: &mut J,
    protocol: Protocol,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Record, JournalError> {
    storage::check(budget, cancelled)?;
    let length = journal.storage_len()?;
    if length < HEADER_BYTES as u64 {
        return Err(JournalError::InvalidJournal);
    }
    let mut header = [0; HEADER_BYTES];
    storage::read_at(journal, 0, &mut header, budget, cancelled)?;
    if &header[..BEFORE_LENGTH_OFFSET] != protocol.magic() {
        return Err(JournalError::InvalidJournal);
    }
    let before = u64::from_le_bytes(
        header[BEFORE_LENGTH_OFFSET..AFTER_LENGTH_OFFSET]
            .try_into()
            .map_err(|_| JournalError::InvalidJournal)?,
    );
    let after = u64::from_le_bytes(
        header[AFTER_LENGTH_OFFSET..DIGEST_OFFSET]
            .try_into()
            .map_err(|_| JournalError::InvalidJournal)?,
    );
    let body_end = protocol.body_end(before, after)?;
    let maximum = body_end + MARKER_BYTES as u64;
    if !(body_end..=maximum).contains(&length) {
        return Err(JournalError::InvalidJournal);
    }

    let before = images::read(
        journal,
        HEADER_BYTES as u64,
        usize::try_from(before).map_err(|_| JournalError::InvalidJournal)?,
        budget,
        cancelled,
    )?;
    let after = images::read(
        journal,
        HEADER_BYTES as u64 + before.len() as u64,
        usize::try_from(after).map_err(|_| JournalError::InvalidJournal)?,
        budget,
        cancelled,
    )?;
    let actual = digest(protocol, &before, &after, budget, cancelled)?;
    if header[DIGEST_OFFSET..] != actual {
        return Err(JournalError::InvalidJournal);
    }
    let mut record = Record {
        protocol,
        before,
        after,
        digest: actual,
        state: State::Pending,
    };
    let count = usize::try_from(length - body_end).map_err(|_| JournalError::InvalidJournal)?;
    let mut marker = [0; MARKER_BYTES];
    storage::read_at(journal, body_end, &mut marker[..count], budget, cancelled)?;
    let expected = record.marker();
    if marker[..count] != expected[..count] {
        return Err(JournalError::InvalidJournal);
    }
    if count == MARKER_BYTES {
        record.state = State::Committed;
    }

    storage::check(budget, cancelled)?;
    Ok(record)
}
