//! Valid generic image journals can still contradict native source-derived kind.
use super::{Storage, budget, fixtures, scope};
use fixtures::View;
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use tablecraft_ace::{
    NativeCompleteInsertIntent, NativeRecoveryQualificationError,
    qualify_native_complete_insert_recovery,
};
use tablecraft_io::{
    IoError,
    journal::{
        CheckedImage, DurableStorage, ResizableDurableStorage, inspect_image, prepare_expected,
        prepare_resizable_expected,
    },
};
use tablecraft_testkit::fixtures::access::canonical_ordinary_wide_insert::Case;

struct DeniedTarget {
    bytes: Cursor<Vec<u8>>,
    writes: usize,
    resizes: usize,
    flushes: usize,
    syncs: usize,
}

impl DeniedTarget {
    fn new(bytes: &[u8]) -> Self {
        Self {
            bytes: Cursor::new(bytes.to_vec()),
            writes: 0,
            resizes: 0,
            flushes: 0,
            syncs: 0,
        }
    }

    fn assert_untouched(&self, original: &[u8]) {
        assert_eq!(self.bytes.get_ref(), original);
        assert_eq!(
            (self.writes, self.resizes, self.flushes, self.syncs),
            (0, 0, 0, 0)
        );
    }
}

impl Read for DeniedTarget {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.bytes.read(output)
    }
}

impl Seek for DeniedTarget {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(position)
    }
}

impl Write for DeniedTarget {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        Err(io::Error::from(io::ErrorKind::PermissionDenied))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        Err(io::Error::from(io::ErrorKind::PermissionDenied))
    }
}

impl DurableStorage for DeniedTarget {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        Ok(self.bytes.get_ref().len() as u64)
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        self.syncs += 1;
        Err(IoError::Io(io::ErrorKind::PermissionDenied))
    }
}

impl ResizableDurableStorage for DeniedTarget {
    fn resize_storage(&mut self, _: u64) -> Result<(), IoError> {
        self.resizes += 1;
        Err(IoError::Io(io::ErrorKind::PermissionDenied))
    }
}

fn reject_equal_v2(view: &View, empty: bool) {
    let after = if empty {
        &view.source.before
    } else {
        &view.source.after
    };
    assert_eq!(view.source.before.len(), after.len());
    let intent = if empty {
        NativeCompleteInsertIntent::new(
            "Items",
            vec![],
            &view.source.before,
            &mut budget(),
            &|| false,
        )
        .unwrap()
    } else {
        fixtures::intent(view)
    };
    let mut target = DeniedTarget::new(&view.source.before);
    let mut journal = Storage::new(Vec::new());
    // Generic V2 explicitly supports independently bounded equal lengths.
    // Preparation reads this denied target and writes only the journal.
    drop(
        prepare_resizable_expected(
            &mut target,
            &mut journal,
            &view.source.before,
            after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    target.assert_untouched(&view.source.before);
    let saved = journal.bytes.get_ref().clone();
    let writes = journal.writes;
    let checked = inspect_image(&mut journal, &mut budget(), &|| false).unwrap();
    match &checked {
        CheckedImage::Resizable(session) => {
            assert_eq!(session.before_bytes(), view.source.before);
            assert_eq!(session.after_bytes(), after);
            assert_eq!(session.before_bytes().len(), session.after_bytes().len());
        }
        CheckedImage::Fixed(_) => panic!("valid V2 was silently changed into V1"),
    }
    // Move this SAME checked session into native qualification. No second
    // inspection, alternate framing or fake journal-integrity failure is used.
    assert!(matches!(
        qualify_native_complete_insert_recovery(checked, intent, scope(), &mut budget(), &|| false,),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));
    target.assert_untouched(&view.source.before);
    assert_eq!(journal.bytes.get_ref(), &saved);
    assert_eq!(journal.writes, writes);
}

#[test]
fn native_complete_kind_refuses_valid_equal_length_v2_for_existing_and_empty_requests() {
    let mut contexts = 0;
    for view in fixtures::cases() {
        if !matches!(view.case, Case::EofFixed) {
            reject_equal_v2(&view, false);
            contexts += 1;
        }
        if matches!(view.case, Case::FixedExact) {
            reject_equal_v2(&view, true);
            contexts += 1;
        }
    }
    assert_eq!(contexts, 16);
}

#[test]
fn native_complete_kind_refuses_valid_v1_when_original_requests_require_one_eof_page() {
    let mut contexts = 0;
    for view in fixtures::cases()
        .into_iter()
        .filter(|view| matches!(view.case, Case::EofFixed))
    {
        assert_eq!(view.source.after.len(), view.source.before.len() + 4096);
        // Both choices are completely valid equal-length generic V1 journals.
        // One contains the truncated literal growing AFTER; the other contains
        // a healthy original image. The original nonempty request still needs
        // an EOF page in both cases, derived from BEFORE capacity and row bytes.
        for after in [
            &view.source.after[..view.source.before.len()],
            &view.source.before[..],
        ] {
            let mut target = DeniedTarget::new(&view.source.before);
            let mut journal = Storage::new(Vec::new());
            drop(
                prepare_expected(
                    &mut target,
                    &mut journal,
                    &view.source.before,
                    after,
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            );
            target.assert_untouched(&view.source.before);
            let saved = journal.bytes.get_ref().clone();
            let writes = journal.writes;
            let checked = inspect_image(&mut journal, &mut budget(), &|| false).unwrap();
            match &checked {
                CheckedImage::Fixed(session) => {
                    assert_eq!(session.before_bytes(), view.source.before);
                    assert_eq!(session.after_bytes(), after);
                    assert_eq!(session.before_bytes().len(), session.after_bytes().len());
                }
                CheckedImage::Resizable(_) => panic!("valid V1 was silently changed into V2"),
            }
            assert!(matches!(
                qualify_native_complete_insert_recovery(
                    checked,
                    fixtures::intent(&view),
                    scope(),
                    &mut budget(),
                    &|| false,
                ),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            target.assert_untouched(&view.source.before);
            assert_eq!(journal.bytes.get_ref(), &saved);
            assert_eq!(journal.writes, writes);
            contexts += 1;
        }
    }
    assert_eq!(contexts, 4);
}
