//! Complete read-only CAB extraction into explicit host-owned storage.
#[path = "deployment_spooled/mod.rs"]
mod boundaries;
#[path = "support/deployment.rs"]
mod fixture;

use std::{
    cell::Cell,
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    rc::Rc,
};
use tablecraft_access::{AccessInput, AccessInputError};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::ResourceBudget;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 1000, 1_000_000)
}

fn database(code: u8) -> Vec<u8> {
    let mut bytes = vec![0; 12 * 4096];
    bytes[..4].copy_from_slice(&[0, 1, 0, 0]);
    bytes[4..19].copy_from_slice(b"Standard ACE DB");
    bytes[20] = code;
    for (index, byte) in bytes[4096..].iter_mut().enumerate() {
        *byte = (index % 251) as u8;
    }
    bytes
}

struct Spool {
    inner: Cursor<Vec<u8>>,
    dropped: Rc<Cell<bool>>,
    written: Rc<Cell<usize>>,
    maximum_write: usize,
    interrupts: usize,
    failure: Option<io::ErrorKind>,
    flush_failure: Option<io::ErrorKind>,
    cancel_on_flush: Option<Rc<Cell<bool>>>,
    seek_mismatch: bool,
    prefix_read: bool,
    final_rewind_failure: Option<io::ErrorKind>,
    cancel_on_final_rewind: Option<Rc<Cell<bool>>>,
}

impl Spool {
    fn fresh(dropped: Rc<Cell<bool>>, written: Rc<Cell<usize>>) -> Self {
        Self {
            inner: Cursor::new(Vec::new()),
            dropped,
            written,
            maximum_write: usize::MAX,
            interrupts: 0,
            failure: None,
            flush_failure: None,
            cancel_on_flush: None,
            seek_mismatch: false,
            prefix_read: false,
            final_rewind_failure: None,
            cancel_on_final_rewind: None,
        }
    }
}

impl Drop for Spool {
    fn drop(&mut self) {
        self.dropped.set(true);
    }
}

impl Read for Spool {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.read(bytes)?;
        self.prefix_read |= count != 0;
        Ok(count)
    }
}

impl Seek for Spool {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        if self.prefix_read && matches!(position, SeekFrom::Start(0)) {
            if let Some(kind) = self.final_rewind_failure {
                return Err(kind.into());
            }
            if let Some(cancelled) = &self.cancel_on_final_rewind {
                cancelled.set(true);
            }
        }
        let actual = self.inner.seek(position)?;
        Ok(actual + u64::from(self.seek_mismatch && matches!(position, SeekFrom::Start(_))))
    }
}

impl Write for Spool {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.interrupts != 0 {
            self.interrupts -= 1;
            return Err(io::ErrorKind::Interrupted.into());
        }
        if let Some(kind) = self.failure {
            return Err(io::Error::new(kind, "private host path"));
        }
        let length = bytes.len().min(self.maximum_write);
        let count = self.inner.write(&bytes[..length])?;
        self.written.set(self.written.get() + count);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(cancelled) = &self.cancel_on_flush {
            cancelled.set(true);
        }
        match self.flush_failure {
            Some(kind) => Err(io::Error::new(kind, "private flush detail")),
            None => Ok(()),
        }
    }
}

#[test]
fn raw_native_never_requests_spool_and_keeps_original_read_seek_bytes() {
    for code in [2, 3, 5, 6] {
        let original = database(code);
        let mut source = AccessInput::<_, Cursor<Vec<u8>>>::open_spooled(
            Cursor::new(&original),
            || panic!("raw source must not request storage"),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(!source.is_deployment());
        let mut actual = Vec::new();
        source.read_to_end(&mut actual).unwrap();
        assert_eq!(actual, original);
        assert_eq!(
            source.seek(SeekFrom::End(-1)).unwrap(),
            original.len() as u64 - 1
        );
    }
}

#[test]
fn stored_and_mszip_all_ace_headers_extract_exact_bytes_with_lazy_owned_storage() {
    for code in [2, 3, 5, 6] {
        let original = database(code);
        for compressed in [false, true] {
            let package = fixture::cabinet(&original, compressed);
            let before = package.clone();
            let factories = Cell::new(0);
            let mut source = AccessInput::open_spooled(
                Cursor::new(&package),
                || {
                    factories.set(factories.get() + 1);
                    Ok(Cursor::new(Vec::new()))
                },
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert!(source.is_deployment());
            assert_eq!(factories.get(), 1);
            let mut actual = Vec::new();
            source.read_to_end(&mut actual).unwrap();
            assert_eq!(actual, original);
            source.seek(SeekFrom::Start(4091)).unwrap();
            let mut crossing = [0; 20];
            source.read_exact(&mut crossing).unwrap();
            assert_eq!(crossing, original[4091..4111]);
            assert_eq!(package, before);
        }
    }
}

#[test]
fn fresh_spool_short_and_interrupted_writes_complete_then_drop_with_source() {
    let original = database(5);
    let package = fixture::cabinet(&original, false);
    let dropped = Rc::new(Cell::new(false));
    let written = Rc::new(Cell::new(0));
    let mut spool = Spool::fresh(dropped.clone(), written.clone());
    spool.maximum_write = 7;
    spool.interrupts = 3;
    let mut source =
        AccessInput::open_spooled(Cursor::new(&package), || Ok(spool), &mut budget(), &|| {
            false
        })
        .unwrap();
    assert!(!dropped.get());
    assert_eq!(written.get(), original.len());
    let mut actual = Vec::new();
    source.read_to_end(&mut actual).unwrap();
    assert_eq!(actual, original);
    drop(source);
    assert!(dropped.get());
}

#[test]
fn hybrid_small_capture_preserves_existing_default_allowance_without_requesting_spool() {
    let original = include_bytes!("../../../../docs/examples/Access_Example_VBA.accdb");
    for compressed in [false, true] {
        let package = fixture::cabinet(original, compressed);
        let mut source = AccessInput::<_, Cursor<Vec<u8>>>::open_with_spool(
            Cursor::new(&package),
            || panic!("small capture must retain memory strategy"),
            &mut ResourceBudget::new(16 * 1024 * 1024, 10000, 1_000_000),
            &|| false,
        )
        .unwrap();
        assert!(source.is_deployment());
        let mut actual = Vec::new();
        source.read_to_end(&mut actual).unwrap();
        assert_eq!(actual.as_slice(), original);
    }
    let mut source = AccessInput::<_, Cursor<Vec<u8>>>::open_with_spool(
        Cursor::new(original),
        || panic!("raw native must not request spool"),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(!source.is_deployment());
    let mut actual = Vec::new();
    source.read_to_end(&mut actual).unwrap();
    assert_eq!(actual.as_slice(), original);
}
