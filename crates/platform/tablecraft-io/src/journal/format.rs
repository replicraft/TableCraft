use super::{
    JournalError,
    storage::{self, BLOCK_BYTES, DurableStorage},
};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MAGIC: &[u8; 8] = b"TCIMG001";
const COMMITTED: &[u8; 8] = b"COMMIT01";
const LENGTH_OFFSET: usize = MAGIC.len();
const DIGEST_OFFSET: usize = LENGTH_OFFSET + size_of::<u64>();
const DIGEST_BYTES: usize = 32; // SHA-256's defined output width.
pub(super) const HEADER_BYTES: usize = DIGEST_OFFSET + DIGEST_BYTES;
pub(super) const MARKER_BYTES: usize = COMMITTED.len() + DIGEST_BYTES;
pub(super) const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum State {
    Pending,
    Committed,
    Original,
}

pub(super) struct Record {
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub digest: [u8; DIGEST_BYTES],
    pub state: State,
}

impl Record {
    pub fn body_end(&self) -> u64 {
        HEADER_BYTES as u64 + 2 * self.before.len() as u64
    }

    pub fn header(&self) -> [u8; HEADER_BYTES] {
        let mut header = [0; HEADER_BYTES];
        header[..LENGTH_OFFSET].copy_from_slice(MAGIC);
        header[LENGTH_OFFSET..DIGEST_OFFSET]
            .copy_from_slice(&(self.before.len() as u64).to_le_bytes());
        header[DIGEST_OFFSET..].copy_from_slice(&self.digest);
        header
    }

    pub fn marker(&self) -> [u8; MARKER_BYTES] {
        let mut marker = [0; MARKER_BYTES];
        marker[..COMMITTED.len()].copy_from_slice(COMMITTED);
        marker[COMMITTED.len()..].copy_from_slice(&self.digest);
        marker
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
    for image in [before, after] {
        for chunk in image.chunks(BLOCK_BYTES) {
            storage::check(budget, cancelled)?;
            budget.charge(Resource::Steps, chunk.len() as u64)?;
            hash.update(chunk);
        }
    }
    Ok(hash.finalize().into())
}

pub(super) fn load<J: DurableStorage>(
    journal: &mut J,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Record, JournalError> {
    storage::check(budget, cancelled)?;
    let journal_len = journal.storage_len()?;
    if journal_len < HEADER_BYTES as u64 {
        return Err(JournalError::InvalidJournal);
    }

    let mut header = [0; HEADER_BYTES];
    storage::read_at(journal, 0, &mut header, budget, cancelled)?;
    if &header[..LENGTH_OFFSET] != MAGIC {
        return Err(JournalError::InvalidJournal);
    }
    let length = u64::from_le_bytes(
        header[LENGTH_OFFSET..DIGEST_OFFSET]
            .try_into()
            .map_err(|_| JournalError::InvalidJournal)?,
    );
    if length == 0 || length > MAX_IMAGE_BYTES {
        return Err(JournalError::InvalidJournal);
    }
    let body_end = HEADER_BYTES as u64 + 2 * length;
    if !(body_end..=body_end + MARKER_BYTES as u64).contains(&journal_len) {
        return Err(JournalError::InvalidJournal);
    }

    let size = usize::try_from(length).map_err(|_| JournalError::InvalidJournal)?;
    let before = storage::read_image(journal, HEADER_BYTES as u64, size, budget, cancelled)?;
    let after = storage::read_image(
        journal,
        HEADER_BYTES as u64 + length,
        size,
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
    let count = (journal_len - body_end) as usize;
    let mut marker = [0; MARKER_BYTES];
    storage::read_at(journal, body_end, &mut marker[..count], budget, cancelled)?;
    let committed = record.marker();
    if count == MARKER_BYTES && marker == committed {
        record.state = State::Committed;
    } else if marker[..count] != committed[..count] {
        return Err(JournalError::InvalidJournal);
    }

    storage::check(budget, cancelled)?;
    Ok(record)
}
