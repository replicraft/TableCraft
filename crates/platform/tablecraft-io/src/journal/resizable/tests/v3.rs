//! Independent V3 framing and version boundaries over the shared transaction core.
use super::{Observed, budget, memory};
use crate::{
    IoError,
    journal::{
        BoundedJournalRegion, CheckedImage, DurableStorage, JournalError, RecoveryOutcome, V3State,
        inspect_image, inspect_v2, inspect_v3, prepare_resizable_expected,
        prepare_resizable_v3_expected, recover_resizable, recover_resizable_v3,
    },
};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
};
use tablecraft_core::limits::ResourceBudget;

fn literal(before: &[u8], after: &[u8], marker: usize) -> Vec<u8> {
    let mut hash = Sha256::new();
    hash.update(b"TCIMG003");
    hash.update((before.len() as u64).to_le_bytes());
    hash.update((after.len() as u64).to_le_bytes());
    hash.update(before);
    hash.update(after);
    let digest = hash.finalize();
    let mut bytes = b"TCIMG003".to_vec();
    bytes.extend((before.len() as u64).to_le_bytes());
    bytes.extend((after.len() as u64).to_le_bytes());
    bytes.extend(digest);
    bytes.extend(before);
    bytes.extend(after);
    let mut committed = b"COMMIT03".to_vec();
    committed.extend(digest);
    bytes.extend(&committed[..marker]);
    bytes
}

#[test]
fn v3_literal_images_marker_prefixes_and_version_discriminators_stay_closed() {
    for case in super::goldens::CASES {
        for marker in 0..=40 {
            let expected = literal(case.before, case.after, marker);
            let mut journal = memory(&expected);
            assert!(inspect_v2(&mut journal, &mut budget(), &|| false).is_err());
            let checked = inspect_v3(&mut journal, &mut budget(), &|| false).unwrap();
            assert_eq!(checked.before_bytes(), case.before);
            assert_eq!(checked.after_bytes(), case.after);
            assert_eq!(
                checked.state(),
                if marker == 40 {
                    V3State::Committed
                } else {
                    V3State::Pending
                }
            );
        }
        let mut old = memory(&super::goldens::bytes(case.pending));
        assert!(inspect_v3(&mut old, &mut budget(), &|| false).is_err());
        let mut journal = memory(&[]);
        let mut target = memory(case.before);
        let prepared = prepare_resizable_v3_expected(
            &mut target,
            &mut journal,
            case.before,
            case.after,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        prepared.commit(&mut budget(), &|| false).unwrap();
        assert_eq!(journal.0.get_ref(), &literal(case.before, case.after, 40));
        assert_eq!(target.0.get_ref(), case.after);
        assert!(recover_resizable(&mut target, &mut journal, &mut budget(), &|| false).is_err());
        assert!(matches!(
            inspect_image(&mut journal, &mut budget(), &|| false).unwrap(),
            CheckedImage::ResizableV3(_)
        ));
    }
}

#[test]
fn v3_mixed_recovery_and_settled_confirmation_use_the_original_journal_owner() {
    for (before, after) in [
        (b"old".as_slice(), b"new-longer".as_slice()),
        (b"before-tail".as_slice(), b"new".as_slice()),
        (b"".as_slice(), b"new".as_slice()),
    ] {
        for committed in [false, true] {
            let mut target = Observed::new(if committed { after } else { before });
            let mut journal = memory(&literal(before, after, if committed { 40 } else { 3 }));
            if !committed && !before.is_empty() {
                target.storage.0.get_mut()[0] = after[0];
            }
            let checked = inspect_v3(&mut journal, &mut budget(), &|| false).unwrap();
            let outcome = checked
                .recover(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                outcome,
                if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                }
            );
            assert_eq!(
                target.storage.0.get_ref(),
                if committed { after } else { before }
            );
            let writes = target.writes;
            let resizes = target.resizes;
            inspect_v3(&mut journal, &mut budget(), &|| false)
                .unwrap()
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(target.writes, writes);
            assert_eq!(target.resizes, resizes);
        }
        let mut journal = memory(&[]);
        let mut target = memory(before);
        prepare_resizable_v3_expected(
            &mut target,
            &mut journal,
            before,
            after,
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .rollback(&mut budget(), &|| false)
        .unwrap();
        assert_eq!(target.0.get_ref(), before);
    }
    let mut wrong = Observed::new(b"alien");
    let mut journal = memory(&literal(b"before", b"after!", 0));
    assert!(recover_resizable_v3(&mut wrong, &mut journal, &mut budget(), &|| false).is_err());
    assert_eq!(wrong.writes, 0);
    assert_eq!(wrong.resizes, 0);
}

#[test]
fn v3_overflow_corruption_budget_and_cancellation_refuse_before_effects() {
    let valid = literal(b"old", b"new-tail", 0);
    for length in 0..valid.len() {
        let mut journal = memory(&valid[..length]);
        assert!(inspect_v3(&mut journal, &mut budget(), &|| false).is_err());
    }
    for offset in [0, 8, 16, 24, 56, 59] {
        let mut bytes = valid.to_vec();
        bytes[offset] ^= 0xff;
        let mut journal = memory(&bytes);
        assert!(inspect_v3(&mut journal, &mut budget(), &|| false).is_err());
    }
    for (before, after) in [(u64::MAX, 0), (0, u64::MAX), (u64::MAX / 2, u64::MAX / 2)] {
        let mut bytes = valid[..56].to_vec();
        bytes[8..16].copy_from_slice(&before.to_le_bytes());
        bytes[16..24].copy_from_slice(&after.to_le_bytes());
        let mut journal = memory(&bytes);
        assert!(matches!(
            inspect_v3(&mut journal, &mut budget(), &|| false),
            Err(JournalError::InvalidJournal)
        ));
    }
    for mut allowance in [
        ResourceBudget::new(0, 0, 100_000),
        ResourceBudget::new(100_000, 0, 0),
    ] {
        let mut target = Observed::new(b"old");
        let mut journal = memory(&[]);
        assert!(
            prepare_resizable_v3_expected(
                &mut target,
                &mut journal,
                b"old",
                b"new",
                &mut allowance,
                &|| false
            )
            .is_err()
        );
        assert_eq!(target.writes, 0);
        assert_eq!(target.resizes, 0);
    }
    let mut journal = memory(&valid);
    assert!(inspect_v3(&mut journal, &mut budget(), &|| true).is_err());
    let mut wrong = memory(&super::goldens::bytes(super::goldens::CASES[0].pending));
    let mut target = Observed::new(b"old");
    assert!(recover_resizable_v3(&mut target, &mut wrong, &mut budget(), &|| false).is_err());
    assert_eq!(target.writes, 0);
}

struct Sparse(File);

impl Read for Sparse {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.0.read(bytes)
    }
}
impl Write for Sparse {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        panic!("inspection must not flush")
    }
}
impl Seek for Sparse {
    fn seek(&mut self, at: SeekFrom) -> io::Result<u64> {
        self.0.seek(at)
    }
}
impl DurableStorage for Sparse {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        Ok(self.0.metadata().unwrap().len())
    }
    fn sync_storage(&mut self) -> Result<(), IoError> {
        panic!("inspection must not sync")
    }
}

#[test]
fn sparse_v3_image_above_legacy_64mib_is_admitted_only_by_new_grammar() {
    // Exceed both the V2 image cap and the former Windows flat-sidecar envelope.
    let length = 2 * 64 * 1024 * 1024 + 4097_u64;
    let mut hash = Sha256::new();
    hash.update(b"TCIMG003");
    hash.update(0_u64.to_le_bytes());
    hash.update(length.to_le_bytes());
    let zeros = [0; 65536];
    let mut remaining = length;
    while remaining != 0 {
        let count = remaining.min(zeros.len() as u64);
        hash.update(&zeros[..count as usize]);
        remaining -= count;
    }
    let prefix = b"host-owned sidecar binding and intent";
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(prefix).unwrap();
    file.write_all(b"TCIMG003").unwrap();
    file.write_all(&0_u64.to_le_bytes()).unwrap();
    file.write_all(&length.to_le_bytes()).unwrap();
    file.write_all(&hash.finalize()).unwrap();
    file.set_len(prefix.len() as u64 + 56 + length).unwrap();
    let mut backing = Sparse(file);
    {
        let mut journal = BoundedJournalRegion::new(
            &mut backing,
            prefix.len() as u64,
            u64::MAX - prefix.len() as u64,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(inspect_v2(&mut journal, &mut budget(), &|| false).is_err());
        let mut enough = ResourceBudget::new(2 * length + 56, 0, 1024 * 1024 * 1024);
        let checked = inspect_v3(&mut journal, &mut enough, &|| false).unwrap();
        assert!(checked.before_bytes().is_empty());
        assert_eq!(checked.after_bytes().len() as u64, length);
        assert!(checked.after_bytes().iter().all(|byte| *byte == 0));
        drop(checked);
    }
    backing.0.seek(SeekFrom::Start(0)).unwrap();
    let mut original = vec![0; prefix.len()];
    backing.0.read_exact(&mut original).unwrap();
    assert_eq!(original, prefix);
    // V2 keeps its explicit 64MiB prepare refusal before trying to own buffers.
    let mut target = memory(b"");
    let after = vec![0; length as usize];
    let mut empty = memory(b"");
    assert!(matches!(
        prepare_resizable_expected(&mut target, &mut empty, b"", &after, &mut budget(), &|| {
            false
        }),
        Err(JournalError::ImageSize)
    ));
    // V3 reaches caller allocation policy instead of a hidden image quota.
    assert!(matches!(
        prepare_resizable_v3_expected(&mut target, &mut empty, b"", &after, &mut budget(), &|| {
            false
        }),
        Err(JournalError::Limit(_))
    ));
}
