use std::io::{self, Read, Seek, SeekFrom, Write};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_io::{IoError, read_exact_at, write_exact_at};

struct ReportedPosition {
    position: u64,
    requested: Option<u64>,
}

impl Seek for ReportedPosition {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let SeekFrom::Start(offset) = from else {
            panic!("absolute I/O must request an absolute offset");
        };
        self.requested = Some(offset);
        Ok(self.position)
    }
}

impl Read for ReportedPosition {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        panic!("must reject the position before reading");
    }
}

impl Write for ReportedPosition {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        panic!("must reject the position before writing");
    }

    fn flush(&mut self) -> io::Result<()> {
        panic!("bounded I/O must not flush");
    }
}

#[test]
fn wrong_seek_positions_refuse_reads_and_writes_even_for_empty_buffers() {
    for (offset, reported) in [(0, 1), (7, 0), (u64::MAX, u64::MAX - 1)] {
        for length in [0, 4] {
            let mut source = ReportedPosition {
                position: reported,
                requested: None,
            };
            let mut budget = ResourceBudget::new(100, 0, 10);
            let mut buffer = [42; 4];
            assert_eq!(
                read_exact_at(
                    &mut source,
                    offset,
                    &mut buffer[..length],
                    &mut budget,
                    &|| false
                ),
                Err(IoError::InvalidSeekPosition)
            );
            assert_eq!(source.requested, Some(offset));
            assert_eq!(buffer, [42; 4]);
            assert_eq!(budget.remaining(Resource::Bytes), 100);
            assert_eq!(budget.remaining(Resource::Steps), 9);

            source.requested = None;
            assert_eq!(
                write_exact_at(&mut source, offset, &buffer[..length], &mut budget, &|| {
                    false
                }),
                Err(IoError::InvalidSeekPosition)
            );
            assert_eq!(source.requested, Some(offset));
            assert_eq!(budget.remaining(Resource::Bytes), 100);
            assert_eq!(budget.remaining(Resource::Steps), 8);
        }
    }
}

#[test]
fn exact_seek_positions_keep_empty_completion_and_pre_seek_cancellation() {
    for offset in [0, 7, u64::MAX] {
        let mut source = ReportedPosition {
            position: offset,
            requested: None,
        };
        let mut budget = ResourceBudget::new(0, 0, 10);
        read_exact_at(&mut source, offset, &mut [], &mut budget, &|| false).unwrap();
        write_exact_at(&mut source, offset, &[], &mut budget, &|| false).unwrap();
        assert_eq!(budget.remaining(Resource::Steps), 4);

        source.requested = None;
        assert_eq!(
            read_exact_at(&mut source, offset, &mut [], &mut budget, &|| true),
            Err(IoError::Cancelled)
        );
        assert_eq!(source.requested, None);
        assert_eq!(
            write_exact_at(&mut source, offset, &[], &mut budget, &|| true),
            Err(IoError::Cancelled)
        );
        assert_eq!(source.requested, None);
        assert_eq!(budget.remaining(Resource::Steps), 4);
    }
}
