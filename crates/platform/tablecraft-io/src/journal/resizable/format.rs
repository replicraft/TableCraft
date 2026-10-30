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
pub(super) enum State {
    Pending,
    Committed,
}

pub(super) struct Record {
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
        bytes[..BEFORE_LENGTH_OFFSET].copy_from_slice(MAGIC);
        bytes[BEFORE_LENGTH_OFFSET..AFTER_LENGTH_OFFSET]
            .copy_from_slice(&(self.before.len() as u64).to_le_bytes());
        bytes[AFTER_LENGTH_OFFSET..DIGEST_OFFSET]
            .copy_from_slice(&(self.after.len() as u64).to_le_bytes());
        bytes[DIGEST_OFFSET..].copy_from_slice(&self.digest);
        bytes
    }

    pub fn marker(&self) -> [u8; MARKER_BYTES] {
        let mut bytes = [0; MARKER_BYTES];
        bytes[..COMMITTED.len()].copy_from_slice(COMMITTED);
        bytes[COMMITTED.len()..].copy_from_slice(&self.digest);
        bytes
    }
}

pub(super) fn digest(
    before: &[u8],
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<[u8; DIGEST_BYTES], JournalError> {
    let mut hash = Sha256::new();
    hash.update(MAGIC);
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
    if &header[..BEFORE_LENGTH_OFFSET] != MAGIC {
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
    if before > MAX_IMAGE_BYTES || after > MAX_IMAGE_BYTES {
        return Err(JournalError::InvalidJournal);
    }
    let body_end = (HEADER_BYTES as u64)
        .checked_add(before)
        .and_then(|size| size.checked_add(after))
        .ok_or(JournalError::InvalidJournal)?;
    let maximum = body_end
        .checked_add(MARKER_BYTES as u64)
        .ok_or(JournalError::InvalidJournal)?;
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
    let actual = digest(&before, &after, budget, cancelled)?;
    if header[DIGEST_OFFSET..] != actual {
        return Err(JournalError::InvalidJournal);
    }
    let mut record = Record {
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
