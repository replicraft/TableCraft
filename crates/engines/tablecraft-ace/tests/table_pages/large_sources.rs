//! Exercise sparse high offsets without allocating or reading a multi-GiB image.
use super::*;
use std::io::{self, Read, Seek, SeekFrom};
use tablecraft_ace::{NativeReadSource, TableCursor};

const SOURCE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const LAST_PAGE: u32 = (SOURCE_BYTES / PAGE_BYTES as u64 - 1) as u32;
const PREFIX_PAGES: usize = 14;

struct SparseSource {
    prefix: Vec<u8>,
    tail: [Vec<u8>; 2],
    position: u64,
    length: u64,
    reads: usize,
    bytes_read: usize,
    tail_reads: [usize; 2],
    fail_last: bool,
}

impl SparseSource {
    fn new(code: u8) -> Self {
        let mut prefix = database(3);
        prefix.resize(PREFIX_PAGES * PAGE_BYTES, 0);
        prefix[20] = code;
        if code != 1 {
            prefix[4..20].copy_from_slice(b"Standard ACE DB\0");
        }

        // Independent direct-map geometry: type, absolute base page, live bits.
        // Both selected data pages are at the extreme end of the virtual source.
        let mut map = vec![0];
        map.extend_from_slice(&(LAST_PAGE - 1).to_le_bytes());
        map.push(0b11);
        prefix[USAGE_PAGE * PAGE_BYTES..(USAGE_PAGE + 1) * PAGE_BYTES]
            .copy_from_slice(&data_page(1, &[(0, map)]));

        Self {
            prefix,
            tail: [
                data_page(TABLE_DEFINITION as u32, &[(0, row(1))]),
                data_page(TABLE_DEFINITION as u32, &[(0, row(2)), (0, row(3))]),
            ],
            position: 0,
            length: SOURCE_BYTES,
            reads: 0,
            bytes_read: 0,
            tail_reads: [0; 2],
            fail_last: false,
        }
    }
}

impl Read for SparseSource {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.reads += 1;
        if buffer.is_empty() || self.position >= self.length {
            return Ok(0);
        }

        let first_tail = u64::from(LAST_PAGE - 1) * PAGE_BYTES as u64;
        let available = if self.position < self.prefix.len() as u64 {
            &self.prefix[self.position as usize..]
        } else if self.position >= first_tail {
            let relative = self.position - first_tail;
            let page = (relative / PAGE_BYTES as u64) as usize;
            if page >= self.tail.len() {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            self.tail_reads[page] += 1;
            if page == 1 && self.fail_last {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
            &self.tail[page][(relative % PAGE_BYTES as u64) as usize..]
        } else {
            // A whole-image read or a scan of unused space is an observable error.
            return Err(io::ErrorKind::InvalidInput.into());
        };
        let count = buffer.len().min(available.len());
        buffer[..count].copy_from_slice(&available[..count]);
        self.position += count as u64;
        self.bytes_read += count;
        Ok(count)
    }
}

impl Seek for SparseSource {
    fn seek(&mut self, requested: SeekFrom) -> io::Result<u64> {
        self.position = match requested {
            SeekFrom::Start(offset) => offset,
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

impl NativeReadSource for SparseSource {}

#[test]
fn complete_and_projected_cursors_read_final_pages_under_small_cumulative_budgets() {
    for code in [1, 2, 5, 6] {
        for projection in [None, Some(&["Score"][..]), Some(&[][..])] {
            let mut source = SparseSource::new(code);
            let mut limits = ResourceBudget::new(256 * 1024, 1024, 100_000);
            {
                let mut cursor = match projection {
                    Some(names) => TableCursor::open_projection(
                        &mut source,
                        SCOPE,
                        "Items",
                        names,
                        &mut limits,
                        &|| false,
                    ),
                    None => TableCursor::open(&mut source, SCOPE, "Items", &mut limits, &|| false),
                }
                .unwrap();

                let mut received = Vec::new();
                loop {
                    let batch = cursor.next_page(1, &mut limits, &|| false).unwrap();
                    received.extend_from_slice(batch.rows());
                    if batch.reached_end() {
                        break;
                    }
                }

                let wanted = match projection {
                    None => expected(0, 3),
                    Some([_]) => (1..=3).map(|id| vec![Value::Int32(id * 10)]).collect(),
                    Some([]) => vec![vec![]; 3],
                    _ => unreachable!(),
                };
                assert_eq!(received, wanted);
            }

            assert_eq!(source.tail_reads, [1, 1]);
            assert!(source.bytes_read < 128 * 1024);
            assert!(source.reads < 64);
            assert!(limits.remaining(Resource::Bytes) > 128 * 1024);
        }
    }
}

#[test]
fn failed_final_page_preserves_the_delivered_prefix_and_closes_continuation() {
    for code in [1, 2, 5, 6] {
        let mut source = SparseSource::new(code);
        source.fail_last = true;
        let mut limits = budget();
        {
            let mut cursor =
                TableCursor::open(&mut source, SCOPE, "Items", &mut limits, &|| false).unwrap();
            let first = cursor.next_page(1, &mut limits, &|| false).unwrap();
            assert_eq!(first.rows(), expected(0, 1));
            assert!(!first.reached_end());
            assert_eq!(cursor.position(), 1);

            assert!(cursor.next_page(10, &mut limits, &|| false).is_err());
            assert_eq!(cursor.position(), 1);
            assert!(cursor.is_failed());
            let remaining = limits.remaining(Resource::Steps);
            assert!(matches!(
                cursor.next_page(1, &mut limits, &|| false),
                Err(InspectError::CursorFailed)
            ));
            assert_eq!(limits.remaining(Resource::Steps), remaining);
        }
        assert_eq!(source.tail_reads, [1, 1]);
    }
}

#[test]
fn source_ceiling_is_checked_before_page_reads() {
    for code in [1, 2, 5, 6] {
        let mut source = SparseSource::new(code);
        source.length += PAGE_BYTES as u64;
        assert!(matches!(
            TableCursor::open(&mut source, SCOPE, "Items", &mut budget(), &|| false),
            Err(InspectError::StructureLimit)
        ));
        assert_eq!(source.reads, 1);
        assert_eq!(source.bytes_read, 66);
        assert_eq!(source.tail_reads, [0, 0]);
    }
}

#[test]
fn usage_map_cannot_advertise_a_page_after_the_source_boundary() {
    for code in [1, 2, 5, 6] {
        let mut source = SparseSource::new(code);
        let mut map = vec![0];
        map.extend_from_slice(&(LAST_PAGE - 1).to_le_bytes());
        map.push(0b111);
        source.prefix[USAGE_PAGE * PAGE_BYTES..(USAGE_PAGE + 1) * PAGE_BYTES]
            .copy_from_slice(&data_page(1, &[(0, map)]));

        assert!(matches!(
            TableCursor::open(&mut source, SCOPE, "Items", &mut budget(), &|| false),
            Err(InspectError::Malformed)
        ));
        assert_eq!(source.tail_reads, [0, 0]);
    }
}

#[test]
fn buffered_materialization_never_returns_a_successful_high_offset_prefix() {
    for code in [1, 2, 5, 6] {
        for corrupt in [false, true] {
            let mut source = SparseSource::new(code);
            if corrupt {
                // The physical owner word must still match the table definition.
                source.tail[1][4..8].copy_from_slice(&999_u32.to_le_bytes());
            } else {
                source.fail_last = true;
            }

            assert!(
                tablecraft_ace::read_table_batched(
                    &mut source,
                    SCOPE,
                    "Items",
                    &mut budget(),
                    &|| false,
                )
                .is_err()
            );
            assert_eq!(source.tail_reads, [1, 1]);
        }
    }
}
