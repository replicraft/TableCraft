#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
mod support;
use std::{
    cell::Cell,
    io::{self, Cursor, Read, Seek, SeekFrom},
};
use tablecraft_ace::{InspectError, NativeImage, inspect_header};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

#[derive(Clone, Copy)]
enum Mode {
    InterruptAt(usize),
    Cancel,
    Stall,
}

struct Source<'a> {
    bytes: Cursor<&'a [u8]>,
    mode: Mode,
    end_calls: usize,
    reads: usize,
    interrupted: bool,
    cancelled: &'a Cell<bool>,
}

impl Read for Source<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.reads += 1;
        self.bytes.read(buffer)
    }
}

impl Seek for Source<'_> {
    fn seek(&mut self, target: SeekFrom) -> io::Result<u64> {
        if matches!(target, SeekFrom::End(0)) {
            self.end_calls += 1;
            match self.mode {
                Mode::InterruptAt(at) if self.end_calls == at => {
                    self.interrupted = true;
                    self.bytes.set_position(1);
                    return Err(io::ErrorKind::Interrupted.into());
                }
                Mode::Cancel => self.cancelled.set(true),
                Mode::Stall => return Err(io::ErrorKind::Interrupted.into()),
                Mode::InterruptAt(_) => {}
            }
        }
        self.bytes.seek(target)
    }
}

fn source<'a>(bytes: &'a [u8], mode: Mode, cancelled: &'a Cell<bool>) -> Source<'a> {
    Source {
        bytes: Cursor::new(bytes),
        mode,
        end_calls: 0,
        reads: 0,
        interrupted: false,
        cancelled,
    }
}

#[test]
fn header_length_inspection_retries_the_same_end_target_without_changing_bytes() {
    let bytes = fixture::database();
    let cancelled = Cell::new(false);
    let mut input = source(&bytes, Mode::InterruptAt(1), &cancelled);
    let header = inspect_header(
        &mut input,
        &mut ResourceBudget::new(1_000_000, 0, 10_000),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        u64::from(header.page_count()) * u64::from(header.page_size()),
        bytes.len() as u64
    );
    assert!(input.interrupted);
    assert_eq!(input.end_calls, 2);
    assert!(input.reads > 0);
    assert_eq!(*input.bytes.get_ref(), bytes.as_slice());
}

#[test]
fn failed_length_inspection_never_reads_or_charges_a_header_prefix() {
    let bytes = fixture::database();
    let cancelled = Cell::new(false);
    let mut input = source(&bytes, Mode::Cancel, &cancelled);
    let mut budget = ResourceBudget::new(1000, 0, 20);
    assert_eq!(
        inspect_header(&mut input, &mut budget, &|| cancelled.get()).err(),
        Some(InspectError::Cancelled)
    );
    assert_eq!(input.reads, 0);
    assert_eq!(budget.remaining(Resource::Bytes), 1000);

    cancelled.set(false);
    let mut input = source(&bytes, Mode::Stall, &cancelled);
    let mut budget = ResourceBudget::new(1000, 0, 5);
    assert!(matches!(
        inspect_header(&mut input, &mut budget, &|| false),
        Err(InspectError::Limit(_))
    ));
    assert_eq!(input.end_calls, 5);
    assert_eq!(input.reads, 0);
    assert_eq!(budget.remaining(Resource::Bytes), 1000);
    assert_eq!(budget.remaining(Resource::Steps), 0);
}

#[test]
fn complete_image_capture_retries_its_final_length_check_without_restarting_capture() {
    let bytes = fixture::database();
    let cancelled = Cell::new(false);
    let mut input = source(&bytes, Mode::InterruptAt(2), &cancelled);
    let image = NativeImage::capture(
        &mut input,
        CatalogScope::new(1),
        &mut ResourceBudget::new(16 * 1024 * 1024, 16_384, 100_000),
        &|| false,
    )
    .unwrap();
    assert!(input.interrupted);
    assert_eq!(image.as_bytes(), bytes.as_slice());
    assert_eq!(*input.bytes.get_ref(), bytes.as_slice());
    assert_eq!(input.end_calls, 3);
}
