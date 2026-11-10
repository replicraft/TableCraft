use super::{CheckedImage, inspect_image};
use crate::{
    IoError,
    journal::{DurableStorage, JournalError, V1State, V2State},
};
use std::{
    cell::{Cell, RefCell},
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    rc::Rc,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

struct ReadOnly(Cursor<Vec<u8>>);

impl Read for ReadOnly {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.0.read(output)
    }
}

impl Seek for ReadOnly {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.0.seek(position)
    }
}

impl Write for ReadOnly {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        panic!("inspection must not write")
    }

    fn flush(&mut self) -> io::Result<()> {
        panic!("inspection must not flush")
    }
}

impl DurableStorage for ReadOnly {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        Ok(self.0.get_ref().len() as u64)
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        panic!("inspection must not synchronize")
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(100_000, 0, 100_000)
}

fn digest(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn exact_discriminator_retains_one_fully_checked_fixed_or_growing_session() {
    // Literal framing and digests, independent of prepare/production encoders.
    let mut fixed = b"TCIMG001\x08\0\0\0\0\0\0\0".to_vec();
    fixed.extend(digest(
        "3721256c8b8e9aca2e0704a11b4e30530ca719eaae4c6ef2c0494601d0d245ac",
    ));
    fixed.extend(b"originalmodified");
    let mut storage = ReadOnly(Cursor::new(fixed));
    match inspect_image(&mut storage, &mut budget(), &|| false).unwrap() {
        CheckedImage::Fixed(checked) => {
            assert_eq!(checked.before_bytes(), b"original");
            assert_eq!(checked.after_bytes(), b"modified");
        }
        CheckedImage::Resizable(_) | CheckedImage::ResizableV3(_) => {
            panic!("fixed framing selected resizable")
        }
    }

    let mut growing = b"TCIMG002\x08\0\0\0\0\0\0\0\x0c\0\0\0\0\0\0\0".to_vec();
    growing.extend(digest(
        "6b35a3e268d3966784aaf48f9cee37cd5cb8660de33a73df34b894caa6416e84",
    ));
    growing.extend(b"originalmodifiedtail");
    let mut storage = ReadOnly(Cursor::new(growing));
    match inspect_image(&mut storage, &mut budget(), &|| false).unwrap() {
        CheckedImage::Resizable(checked) => {
            assert_eq!(checked.before_bytes(), b"original");
            assert_eq!(checked.after_bytes(), b"modifiedtail");
        }
        CheckedImage::Fixed(_) | CheckedImage::ResizableV3(_) => {
            panic!("V2 framing selected another protocol")
        }
    }
}

#[test]
fn unknown_short_or_corrupt_known_framing_refuses_without_an_alternate_decoder() {
    for raw in [
        Vec::new(),
        b"TCIMG00".to_vec(),
        b"TCIMG099".to_vec(),
        b"TCIMG001".to_vec(),
        b"TCIMG002".to_vec(),
    ] {
        let mut storage = ReadOnly(Cursor::new(raw));
        assert!(matches!(
            inspect_image(&mut storage, &mut budget(), &|| false),
            Err(JournalError::InvalidJournal)
        ));
    }
}

#[derive(Default)]
struct Trace {
    reads: RefCell<Vec<(u64, usize)>>,
    lengths: Cell<usize>,
    denied: Cell<bool>,
}

struct Traced {
    bytes: Cursor<Vec<u8>>,
    trace: Rc<Trace>,
}

impl Read for Traced {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        assert!(
            !self.trace.denied.get(),
            "checked views reloaded the journal"
        );
        self.trace
            .reads
            .borrow_mut()
            .push((self.bytes.position(), output.len()));
        self.bytes.read(output)
    }
}

impl Seek for Traced {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(position)
    }
}

impl Write for Traced {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        panic!("inspection must not write")
    }

    fn flush(&mut self) -> io::Result<()> {
        panic!("inspection must not flush")
    }
}

impl DurableStorage for Traced {
    fn storage_len(&mut self) -> Result<u64, IoError> {
        self.trace.lengths.set(self.trace.lengths.get() + 1);
        Ok(self.bytes.get_ref().len() as u64)
    }

    fn sync_storage(&mut self) -> Result<(), IoError> {
        panic!("inspection must not synchronize")
    }
}

fn traced(bytes: Vec<u8>) -> (Traced, Rc<Trace>) {
    let trace = Rc::new(Trace::default());
    (
        Traced {
            bytes: Cursor::new(bytes),
            trace: trace.clone(),
        },
        trace,
    )
}

fn literal(growing: bool, marker_bytes: usize) -> Vec<u8> {
    assert!(marker_bytes <= 40);
    let mut raw = if growing {
        b"TCIMG002\x08\0\0\0\0\0\0\0\x0c\0\0\0\0\0\0\0".to_vec()
    } else {
        b"TCIMG001\x08\0\0\0\0\0\0\0".to_vec()
    };
    let hash = digest(if growing {
        "6b35a3e268d3966784aaf48f9cee37cd5cb8660de33a73df34b894caa6416e84"
    } else {
        "3721256c8b8e9aca2e0704a11b4e30530ca719eaae4c6ef2c0494601d0d245ac"
    });
    raw.extend_from_slice(&hash);
    raw.extend_from_slice(b"original");
    raw.extend_from_slice(if growing {
        b"modifiedtail"
    } else {
        b"modified"
    });
    let body = raw.len();
    assert_eq!(body, if growing { 76 } else { 64 });
    raw.extend_from_slice(if growing { b"COMMIT02" } else { b"COMMIT01" });
    raw.extend_from_slice(&hash);
    raw.truncate(body + marker_bytes);
    raw
}

#[test]
fn every_body_truncation_and_committed_byte_corruption_refuses_but_marker_prefixes_remain_pending()
{
    for growing in [false, true] {
        let complete = literal(growing, 40);
        let body = if growing { 76 } else { 64 };
        for length in 0..body {
            let (mut bytes, _) = traced(complete[..length].to_vec());
            assert!(matches!(
                inspect_image(&mut bytes, &mut budget(), &|| false),
                Err(JournalError::InvalidJournal)
            ));
        }
        for offset in 0..complete.len() {
            let mut changed = complete.clone();
            changed[offset] ^= 1;
            let (mut bytes, _) = traced(changed);
            assert!(
                matches!(
                    inspect_image(&mut bytes, &mut budget(), &|| false),
                    Err(JournalError::InvalidJournal)
                ),
                "growing {growing}, byte {offset}"
            );
        }
        for count in 0..=40 {
            let (mut bytes, trace) = traced(literal(growing, count));
            let checked = inspect_image(&mut bytes, &mut budget(), &|| false).unwrap();
            trace.denied.set(true);
            match checked {
                CheckedImage::Fixed(session) => {
                    assert!(!growing);
                    assert_eq!(session.before_bytes(), b"original");
                    assert_eq!(session.after_bytes(), b"modified");
                    assert_eq!(
                        session.state(),
                        if count == 40 {
                            V1State::Committed
                        } else {
                            V1State::Pending
                        }
                    );
                }
                CheckedImage::Resizable(session) => {
                    assert!(growing);
                    assert_eq!(session.before_bytes(), b"original");
                    assert_eq!(session.after_bytes(), b"modifiedtail");
                    assert_eq!(
                        session.state(),
                        if count == 40 {
                            V2State::Committed
                        } else {
                            V2State::Pending
                        }
                    );
                }
                CheckedImage::ResizableV3(_) => panic!("old framing selected V3"),
            }
            assert_eq!(trace.lengths.get(), 2);
        }
    }
}

#[test]
fn known_corrupt_frame_selects_one_inspector_and_unknown_magic_selects_none() {
    for (magic, header) in [(b"TCIMG001", 48), (b"TCIMG002", 56), (b"TCIMG003", 56)] {
        let mut zero_length = vec![0; 80];
        zero_length[..8].copy_from_slice(magic);
        let (mut bytes, trace) = traced(zero_length);
        assert!(matches!(
            inspect_image(&mut bytes, &mut budget(), &|| false),
            Err(JournalError::InvalidJournal)
        ));
        // One discriminator read and one selected header read. No alternate
        // header, repeated discriminator or third storage-length query occurs.
        assert_eq!(&*trace.reads.borrow(), &[(0, 8), (0, header)]);
        assert_eq!(trace.lengths.get(), 2);
    }
    let (mut bytes, trace) = traced(b"TCIMG004xxxxxxxxxxxxxxxx".to_vec());
    assert!(matches!(
        inspect_image(&mut bytes, &mut budget(), &|| false),
        Err(JournalError::InvalidJournal)
    ));
    assert_eq!(&*trace.reads.borrow(), &[(0, 8)]);
    assert_eq!(trace.lengths.get(), 1);
}

#[test]
fn selected_inspection_exact_pools_one_less_and_every_real_poll_boundary_remain_read_only() {
    for growing in [false, true] {
        let original = literal(growing, 40);
        let (mut bytes, _) = traced(original.clone());
        let mut measured = budget();
        let initial = [Resource::Bytes, Resource::Steps].map(|pool| measured.remaining(pool));
        let polls = Cell::new(0_usize);
        drop(
            inspect_image(&mut bytes, &mut measured, &|| {
                polls.set(polls.get() + 1);
                false
            })
            .unwrap(),
        );
        let used = [
            initial[0] - measured.remaining(Resource::Bytes),
            initial[1] - measured.remaining(Resource::Steps),
        ];
        assert!(used.iter().all(|value| *value > 0));
        let (mut bytes, _) = traced(original.clone());
        let mut exact = ResourceBudget::new(used[0], 0, used[1]);
        drop(inspect_image(&mut bytes, &mut exact, &|| false).unwrap());
        assert_eq!(
            [Resource::Bytes, Resource::Rows, Resource::Steps].map(|pool| exact.remaining(pool)),
            [0, 0, 0]
        );
        for index in 0..2 {
            let mut limits = used;
            limits[index] -= 1;
            let (mut bytes, _) = traced(original.clone());
            let error = inspect_image(
                &mut bytes,
                &mut ResourceBudget::new(limits[0], 0, limits[1]),
                &|| false,
            )
            .err()
            .unwrap();
            let resource = match error {
                JournalError::Limit(error) | JournalError::Io(IoError::Limit(error)) => {
                    error.resource
                }
                other => panic!("expected pool limit, got {other:?}"),
            };
            assert_eq!(resource, [Resource::Bytes, Resource::Steps][index]);
            assert_eq!(bytes.bytes.get_ref(), &original);
        }
        for stop in 1..=polls.get() {
            let seen = Cell::new(0_usize);
            let (mut bytes, _) = traced(original.clone());
            let error = inspect_image(&mut bytes, &mut budget(), &|| {
                seen.set(seen.get() + 1);
                seen.get() >= stop
            })
            .err()
            .unwrap();
            assert_eq!(error, JournalError::Io(IoError::Cancelled));
            assert_eq!(seen.get(), stop);
            assert_eq!(bytes.bytes.get_ref(), &original);
        }
    }
}
