#[path = "../../../tests/support/agile.rs"]
mod fixture;

use super::*;
use crate::{encryption_rewrite_budget, rewrite_encryption_to};
use rayon::ThreadPoolBuilder;
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    fs,
    io::{self, Cursor, SeekFrom},
    path::Path,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const PASSWORD: &str = "public password";
const MEMORY: usize = 1024 * 1024;

fn plain() -> Vec<u8> {
    fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap()
}

pub(super) fn image() -> Vec<u8> {
    fixture::encrypted_image(&plain(), 5, PASSWORD)
}

fn budget(bytes: usize) -> ResourceBudget {
    encryption_rewrite_budget(bytes as u64).unwrap()
}

fn remove() -> EncryptionRewrite<'static> {
    EncryptionRewrite::Remove {
        password: Some(PASSWORD),
    }
}

fn sequential(bytes: &[u8], operation: EncryptionRewrite<'_>) -> Vec<u8> {
    let mut target = Cursor::new(Vec::new());
    rewrite_encryption_to(
        &mut Cursor::new(bytes),
        &mut target,
        operation,
        &mut budget(bytes.len()),
        &|| false,
    )
    .unwrap();
    target.into_inner()
}

fn parallel(bytes: &[u8], operation: EncryptionRewrite<'_>, pool: &ThreadPool) -> Vec<u8> {
    let mut target = Cursor::new(Vec::new());
    let mut memory = |_| Ok(MEMORY);
    rewrite_encryption_parallel_to(
        &mut Cursor::new(bytes),
        &mut target,
        operation,
        ParallelEncryption::new(pool, &mut memory),
        &mut budget(bytes.len()),
        &|| false,
    )
    .unwrap();
    target.into_inner()
}

#[test]
fn adaptive_removal_matches_all_bytes_at_one_and_multiple_workers() {
    let encrypted = image();
    let expected = sequential(&encrypted, remove());
    for threads in [1, 2, 4, 8] {
        let pool = ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let calls = Cell::new(0_usize);
        let mut memory = |retained| {
            calls.set(calls.get() + 1);
            assert!(retained >= FIXED_BUFFER_BYTES);
            Ok(if calls.get().is_multiple_of(2) {
                256 * 1024
            } else {
                MEMORY
            })
        };
        let mut target = Cursor::new(Vec::new());
        rewrite_encryption_parallel_to(
            &mut Cursor::new(&encrypted),
            &mut target,
            remove(),
            ParallelEncryption::new(&pool, &mut memory),
            &mut budget(encrypted.len()),
            &|| false,
        )
        .unwrap();
        assert!(calls.get() > 2);
        assert_eq!(target.into_inner(), expected);
    }
}

#[test]
fn add_and_password_change_retain_existing_jet_and_agile_semantics() {
    let pool = ThreadPoolBuilder::new().num_threads(4).build().unwrap();
    for header in [1, 2, 3, 5, 6] {
        let mut original = plain();
        original[20] = header;
        if header == 1 {
            original[4..20].copy_from_slice(b"Standard Jet DB\0");
        }
        let add = || EncryptionRewrite::Add {
            password: None,
            new_password: PASSWORD,
        };
        let reference = sequential(&sequential(&original, add()), remove());
        let encrypted = parallel(&original, add(), &pool);
        assert_eq!(sequential(&encrypted, remove()), reference);
        let changed = parallel(
            &encrypted,
            EncryptionRewrite::ChangePassword {
                password: Some(PASSWORD),
                new_password: "new public password",
            },
            &pool,
        );
        assert_eq!(
            sequential(
                &changed,
                EncryptionRewrite::Remove {
                    password: Some("new public password"),
                }
            ),
            reference
        );
    }
}

#[test]
fn agile_envelope_only_change_batches_unchanged_ciphertext_without_crypto_scratch() {
    let encrypted = image();
    let pool = ThreadPoolBuilder::new().num_threads(2).build().unwrap();
    let copy_ceiling = FIXED_BUFFER_BYTES + 2 * (PAGE_BYTES + size_of::<PageResult>());
    assert!(copy_ceiling < ParallelEncryption::minimum_batch_bytes());
    let batches = Cell::new(0);
    let mut memory = |retained| {
        batches.set(batches.get() + 1);
        assert!(retained <= copy_ceiling);
        Ok(copy_ceiling)
    };
    let mut target = Cursor::new(Vec::new());
    rewrite_encryption_parallel_to(
        &mut Cursor::new(&encrypted),
        &mut target,
        EncryptionRewrite::ChangePassword {
            password: Some(PASSWORD),
            new_password: "replacement",
        },
        ParallelEncryption::new(&pool, &mut memory),
        &mut budget(encrypted.len()),
        &|| false,
    )
    .unwrap();
    let data_pages = encrypted.len() / PAGE_BYTES - 1;
    assert_eq!(batches.get(), data_pages.div_ceil(2));
    assert_eq!(&target.get_ref()[PAGE_BYTES..], &encrypted[PAGE_BYTES..]);
    assert_eq!(
        sequential(
            target.get_ref(),
            EncryptionRewrite::Remove {
                password: Some("replacement")
            }
        ),
        sequential(&encrypted, remove())
    );
}

#[test]
fn bad_credentials_memory_quotas_and_actual_final_cancellation_never_succeed() {
    let encrypted = image();
    let pool = ThreadPoolBuilder::new().num_threads(4).build().unwrap();
    let mut memory = |_| Ok(MEMORY);
    let mut target = Cursor::new(Vec::new());
    let bad = rewrite_encryption_parallel_to(
        &mut Cursor::new(&encrypted),
        &mut target,
        EncryptionRewrite::Remove {
            password: Some("wrong"),
        },
        ParallelEncryption::new(&pool, &mut memory),
        &mut budget(encrypted.len()),
        &|| false,
    );
    assert!(matches!(bad, Err(InspectError::IncorrectPassword)));
    assert!(target.get_ref().is_empty());

    let mut target = Cursor::new(Vec::new());
    let mut too_small = |_| Ok(ParallelEncryption::minimum_batch_bytes() - 1);
    let result = rewrite_encryption_parallel_to(
        &mut Cursor::new(&encrypted),
        &mut target,
        remove(),
        ParallelEncryption::new(&pool, &mut too_small),
        &mut budget(encrypted.len()),
        &|| false,
    );
    assert!(matches!(result, Err(InspectError::Allocation)));
    assert!(target.get_ref().len() < encrypted.len());

    let calls = AtomicUsize::new(0);
    let mut measured = budget(encrypted.len());
    let before_bytes = measured.remaining(Resource::Bytes);
    let before_steps = measured.remaining(Resource::Steps);
    rewrite_encryption_parallel_to(
        &mut Cursor::new(&encrypted),
        &mut Cursor::new(Vec::new()),
        remove(),
        ParallelEncryption::new(&pool, &mut memory),
        &mut measured,
        &|| {
            calls.fetch_add(1, Ordering::Relaxed);
            false
        },
    )
    .unwrap();
    let final_check = calls.load(Ordering::Relaxed);
    for boundary in [1, final_check / 2, final_check] {
        calls.store(0, Ordering::Relaxed);
        let mut source = Cursor::new(encrypted.clone());
        let mut target = Cursor::new(Vec::new());
        let result = rewrite_encryption_parallel_to(
            &mut source,
            &mut target,
            remove(),
            ParallelEncryption::new(&pool, &mut memory),
            &mut budget(encrypted.len()),
            &|| calls.fetch_add(1, Ordering::Relaxed) + 1 == boundary,
        );
        assert!(
            matches!(result, Err(InspectError::Cancelled)),
            "boundary {boundary}"
        );
        assert_eq!(source.into_inner(), encrypted);
    }
    let used_bytes = before_bytes - measured.remaining(Resource::Bytes);
    let used_steps = before_steps - measured.remaining(Resource::Steps);
    for mut limits in [
        ResourceBudget::new(used_bytes - 1, 100_000, before_steps),
        ResourceBudget::new(before_bytes, 100_000, used_steps - 1),
    ] {
        let result = rewrite_encryption_parallel_to(
            &mut Cursor::new(&encrypted),
            &mut Cursor::new(Vec::new()),
            remove(),
            ParallelEncryption::new(&pool, &mut memory),
            &mut limits,
            &|| false,
        );
        assert!(matches!(result, Err(InspectError::Limit(_))));
    }
}

// Source/target deliberately retain Rc state and are not Send. The coordinator
// alone touches them; the unallocated encrypted tail is a codec/streaming witness,
// not qualification of additional user-table pages or native Access behavior.
struct ExtendedSource<'a> {
    prefix: &'a [u8],
    length: u64,
    position: u64,
    largest_read: Rc<Cell<usize>>,
}

impl Read for ExtendedSource<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.largest_read
            .set(self.largest_read.get().max(bytes.len()));
        let count = bytes
            .len()
            .min(self.length.saturating_sub(self.position) as usize);
        bytes[..count].fill(0);
        if self.position < self.prefix.len() as u64 {
            let start = self.position as usize;
            let copied = count.min(self.prefix.len() - start);
            bytes[..copied].copy_from_slice(&self.prefix[start..start + copied]);
        }
        self.position += count as u64;
        Ok(count)
    }
}

impl Seek for ExtendedSource<'_> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.position = match from {
            SeekFrom::Start(position) => position,
            SeekFrom::Current(delta) => self
                .position
                .checked_add_signed(delta)
                .ok_or(io::ErrorKind::InvalidInput)?,
            SeekFrom::End(delta) => self
                .length
                .checked_add_signed(delta)
                .ok_or(io::ErrorKind::InvalidInput)?,
        };
        Ok(self.position)
    }
}

struct HashTarget {
    hash: Sha256,
    length: Rc<Cell<u64>>,
    position: u64,
}

impl Write for HashTarget {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        assert_eq!(
            self.position,
            self.length.get(),
            "coordinator reordered output"
        );
        self.hash.update(bytes);
        self.position += bytes.len() as u64;
        self.length.set(self.position);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Seek for HashTarget {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.position = match from {
            SeekFrom::Start(position) => position,
            SeekFrom::End(0) => self.length.get(),
            _ => return Err(io::ErrorKind::Unsupported.into()),
        };
        Ok(self.position)
    }
}

#[test]
fn complete_eighty_mib_rewrite_is_batched_and_accepts_coordinator_only_io() {
    let encrypted = image();
    let length = 80 * 1024 * 1024;
    let pool = ThreadPoolBuilder::new().num_threads(4).build().unwrap();
    let mut hashes = Vec::new();
    for parallel in [false, true] {
        let largest_read = Rc::new(Cell::new(0));
        let mut source = ExtendedSource {
            prefix: &encrypted,
            length,
            position: 0,
            largest_read: Rc::clone(&largest_read),
        };
        let written = Rc::new(Cell::new(0));
        let mut target = HashTarget {
            hash: Sha256::new(),
            length: Rc::clone(&written),
            position: 0,
        };
        let mut limits = encryption_rewrite_budget(length).unwrap();
        let batches = Cell::new(0);
        let mut memory = |_| {
            batches.set(batches.get() + 1);
            Ok(MEMORY)
        };
        if parallel {
            rewrite_encryption_parallel_to(
                &mut source,
                &mut target,
                remove(),
                ParallelEncryption::new(&pool, &mut memory),
                &mut limits,
                &|| false,
            )
            .unwrap();
            assert!(batches.get() > 64);
        } else {
            rewrite_encryption_to(&mut source, &mut target, remove(), &mut limits, &|| false)
                .unwrap();
        }
        assert_eq!(written.get(), length);
        assert!(largest_read.get() <= MEMORY);
        hashes.push(target.hash.finalize().to_vec());
    }
    assert_eq!(hashes[0], hashes[1]);
}

#[test]
fn native_two_gib_envelope_is_not_an_image_capture_limit() {
    let encrypted = image();
    let length = 2 * 1024 * 1024 * 1024;
    let pool = ThreadPoolBuilder::new().num_threads(2).build().unwrap();
    let mut source = ExtendedSource {
        prefix: &encrypted,
        length,
        position: 0,
        largest_read: Rc::new(Cell::new(0)),
    };
    let mut target = Cursor::new(Vec::new());
    let calls = Cell::new(0_usize);
    let mut memory = |_| {
        calls.set(calls.get() + 1);
        if calls.get() == 2 {
            Err(InspectError::Cancelled)
        } else {
            Ok(ParallelEncryption::minimum_batch_bytes())
        }
    };
    let result = rewrite_encryption_parallel_to(
        &mut source,
        &mut target,
        remove(),
        ParallelEncryption::new(&pool, &mut memory),
        &mut encryption_rewrite_budget(length).unwrap(),
        &|| false,
    );
    assert!(matches!(result, Err(InspectError::Cancelled)));
    assert_eq!(calls.get(), 2);
    assert_eq!(target.get_ref().len(), 2 * PAGE_BYTES);
    assert!(matches!(
        encryption_rewrite_budget(length + PAGE_BYTES as u64),
        Err(InspectError::StructureLimit)
    ));
}

#[test]
fn bounded_memory_geometry_reuses_without_geometric_shrink_and_can_release_capacity() {
    let mut buffers = buffer::Buffer::new();
    let mut limits = budget(1024 * 1024);
    let pages = buffer::pages_for_memory(MEMORY, 4, 10_000).unwrap();
    buffers.prepare(pages, 4, MEMORY, &mut limits).unwrap();
    let retained = buffers.retained_bytes().unwrap();
    assert!(retained <= MEMORY);
    assert_eq!(buffer::pages_for_memory(MEMORY, 4, 10_000).unwrap(), pages);
    buffers.data.fill(0x65);
    buffers
        .prepare(1, 4, ParallelEncryption::minimum_batch_bytes(), &mut limits)
        .unwrap();
    assert_eq!(buffers.data.len(), PAGE_BYTES);
    assert!(buffers.data.iter().all(|byte| *byte == 0));
    assert!(buffers.retained_bytes().unwrap() < retained);
}
