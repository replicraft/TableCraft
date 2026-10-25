//! Sparse 2 GiB payload locality and atomic continuation, without a large image.
use super::*;
use std::{
    collections::BTreeMap,
    io::{self, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_ace::{NativeReadSource, TableCursor, TableSnapshotLimits};

const SOURCE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const LAST_PAGE: u32 = (SOURCE_BYTES / PAGE_BYTES as u64 - 1) as u32;
const FIRST_MEMO: u32 = LAST_PAGE;
const FIRST_BLOB: u32 = LAST_PAGE - 2;
const MEMO_CHAIN: [u32; 3] = [LAST_PAGE - 9, 14, LAST_PAGE - 5];
const BLOB_CHAIN: [u32; 2] = [LAST_PAGE - 13, LAST_PAGE - 1];
const FINAL_MEMO: u32 = LAST_PAGE - 17;
const FIELD_START: usize = 63;
const FIELD_BYTES: usize = 25;
const EXTENDED_FLAGS: usize = 16;
const PAGE_OWNER: usize = 4;
const MEMO_BYTES: &[u8] = &[0xff, 0xfe, b'A', 0, 0x2d, 0x4e, 0, b'B'];
const BLOB_BYTES: &[u8] = &[0, 255, 1, 128, 0, 66];

#[derive(Clone, Copy, Debug)]
enum PayloadFault {
    WrongOwner,
    ShortChain,
    Cycle,
    MissingPage,
}

struct SparsePayloads<'a> {
    prefix: Vec<u8>,
    pages: BTreeMap<u32, Vec<u8>>,
    position: u64,
    reads: Rc<Cell<usize>>,
    payload_reads: Rc<Cell<usize>>,
    cancelled: &'a Cell<bool>,
    cancel_page: Option<u32>,
}

fn descriptor(tag: u32, length: usize, page: u32) -> Vec<u8> {
    [
        (tag | length as u32).to_le_bytes(),
        (page << 8).to_le_bytes(),
        0_u32.to_le_bytes(),
    ]
    .concat()
}

fn payload_page(bytes: &[u8]) -> Vec<u8> {
    let mut page = data_page(0, &[(0, bytes.to_vec())]);
    page[4..8].copy_from_slice(b"LVAL");
    page
}

fn payload_row(id: i32, memo: &[u8], blob: &[u8]) -> Vec<u8> {
    let mut row = 3_u16.to_le_bytes().to_vec();
    row.extend(id.to_le_bytes());
    let memo_start = row.len() as u16;
    row.extend(memo);
    let blob_start = row.len() as u16;
    row.extend(blob);
    row.extend((row.len() as u16).to_le_bytes());
    row.extend(blob_start.to_le_bytes());
    row.extend(memo_start.to_le_bytes());
    row.extend(2_u16.to_le_bytes());
    row.push(7);
    row
}

impl<'a> SparsePayloads<'a> {
    fn new(code: u8, cancelled: &'a Cell<bool>) -> Self {
        let mut prefix = database(3);
        prefix[20] = code;
        if code != 1 {
            prefix[4..20].copy_from_slice(b"Standard ACE DB\0");
        }

        // Build two variable long-value slots independently of reader constants.
        let mut schema = fixture::definition(
            &[("Id", 4, 0, 4), ("Memo", 11, 0, 0), ("Blob", 11, 0, 0)],
            3,
            11 << 8,
        );
        schema[FIELD_START + FIELD_BYTES] = 12;
        schema[FIELD_START + FIELD_BYTES + EXTENDED_FLAGS] = 1;
        prefix[10 * PAGE_BYTES..11 * PAGE_BYTES].copy_from_slice(&schema);
        let first_memo = descriptor(0x4000_0000, MEMO_BYTES.len(), FIRST_MEMO);
        let first_blob = descriptor(0x4000_0000, BLOB_BYTES.len(), FIRST_BLOB);
        let rows = vec![
            (0, payload_row(1, &first_memo, &first_blob)),
            (
                0,
                payload_row(
                    2,
                    &descriptor(0, MEMO_BYTES.len(), MEMO_CHAIN[0]),
                    &descriptor(0, BLOB_BYTES.len(), BLOB_CHAIN[0]),
                ),
            ),
            (
                0,
                payload_row(3, &descriptor(0, MEMO_BYTES.len(), FINAL_MEMO), &first_blob),
            ),
        ];
        prefix[12 * PAGE_BYTES..13 * PAGE_BYTES].copy_from_slice(&data_page(10, &rows));

        let mut pages = BTreeMap::new();
        pages.insert(FIRST_MEMO, payload_page(MEMO_BYTES));
        pages.insert(FIRST_BLOB, payload_page(BLOB_BYTES));
        for (index, chunk) in [&MEMO_BYTES[..4], &MEMO_BYTES[4..6], &MEMO_BYTES[6..]]
            .into_iter()
            .enumerate()
        {
            let next = MEMO_CHAIN.get(index + 1).copied().unwrap_or(0) << 8;
            let bytes = [next.to_le_bytes().as_slice(), chunk].concat();
            pages.insert(MEMO_CHAIN[index], payload_page(&bytes));
        }
        for (index, chunk) in BLOB_BYTES.chunks(3).enumerate() {
            let next = BLOB_CHAIN.get(index + 1).copied().unwrap_or(0) << 8;
            let bytes = [next.to_le_bytes().as_slice(), chunk].concat();
            pages.insert(BLOB_CHAIN[index], payload_page(&bytes));
        }
        pages.insert(
            FINAL_MEMO,
            payload_page(&[0_u32.to_le_bytes().as_slice(), MEMO_BYTES].concat()),
        );

        Self {
            prefix,
            pages,
            position: 0,
            reads: Rc::new(Cell::new(0)),
            payload_reads: Rc::new(Cell::new(0)),
            cancelled,
            cancel_page: None,
        }
    }
}

impl Read for SparsePayloads<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.reads.set(self.reads.get() + 1);
        if buffer.is_empty() || self.position >= SOURCE_BYTES {
            return Ok(0);
        }

        let page = (self.position / PAGE_BYTES as u64) as u32;
        let offset = (self.position % PAGE_BYTES as u64) as usize;
        let available = if self.position < self.prefix.len() as u64 {
            &self.prefix[self.position as usize..]
        } else {
            self.payload_reads.set(self.payload_reads.get() + 1);
            self.pages
                .get(&page)
                .ok_or(io::ErrorKind::PermissionDenied)?
                .get(offset..)
                .ok_or(io::ErrorKind::UnexpectedEof)?
        };
        let count = buffer.len().min(available.len());
        buffer[..count].copy_from_slice(&available[..count]);
        self.position += count as u64;
        Ok(count)
    }
}

impl Seek for SparsePayloads<'_> {
    fn seek(&mut self, requested: SeekFrom) -> io::Result<u64> {
        self.position = match requested {
            SeekFrom::Start(offset) => offset,
            SeekFrom::Current(delta) => self
                .position
                .checked_add_signed(delta)
                .ok_or(io::ErrorKind::InvalidInput)?,
            SeekFrom::End(delta) => SOURCE_BYTES
                .checked_add_signed(delta)
                .ok_or(io::ErrorKind::InvalidInput)?,
        };
        Ok(self.position)
    }
}

impl NativeReadSource for SparsePayloads<'_> {
    fn decode_native_page(
        &mut self,
        number: u32,
        page: &mut [u8],
        _: &mut ResourceBudget,
        _: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        if self.cancel_page == Some(number) {
            page.fill(0);
            self.cancelled.set(true);
        }
        Ok(())
    }
}

fn expected_payloads(id: i32) -> Vec<Value> {
    vec![
        Value::Int32(id),
        Value::Text("A中B".into()),
        Value::Binary(BLOB_BYTES.to_vec()),
    ]
}

#[test]
fn sparse_final_page_and_nonadjacent_chains_decode_exact_memo_and_ole_values() {
    for code in [1, 2, 5, 6] {
        let cancelled = Cell::new(false);
        let mut source = SparsePayloads::new(code, &cancelled);
        let original = (source.prefix.clone(), source.pages.clone());
        let mut limits = ResourceBudget::new(256 * 1024, 1024, 100_000);
        {
            let mut cursor =
                TableCursor::open(&mut source, SCOPE, "Items", &mut limits, &|| false).unwrap();
            for id in 1..=3 {
                let batch = cursor.next_page(1, &mut limits, &|| false).unwrap();
                assert_eq!(batch.rows(), [expected_payloads(id)]);
            }
            assert!(
                cursor
                    .next_page(1, &mut limits, &|| false)
                    .unwrap()
                    .reached_end()
            );
        }
        assert!(source.payload_reads.get() >= 9);
        assert!(source.reads.get() < 64);
        assert!(limits.remaining(Resource::Bytes) > 128 * 1024);
        assert_eq!((source.prefix, source.pages), original);
    }
}

#[test]
fn id_and_empty_projections_do_not_visit_even_corrupt_external_payloads() {
    for code in [1, 2, 5, 6] {
        for names in [&["Id"][..], &[][..]] {
            let cancelled = Cell::new(false);
            let mut source = SparsePayloads::new(code, &cancelled);
            source.pages.clear();
            let mut limits = budget();
            {
                let mut cursor = TableCursor::open_projection(
                    &mut source,
                    SCOPE,
                    "Items",
                    names,
                    &mut limits,
                    &|| false,
                )
                .unwrap();
                let batch = cursor.next_page(3, &mut limits, &|| false).unwrap();
                let wanted: Vec<Vec<Value>> = (1..=3)
                    .map(|id| {
                        if names.is_empty() {
                            vec![]
                        } else {
                            vec![Value::Int32(id)]
                        }
                    })
                    .collect();
                assert_eq!(batch.rows(), wanted);
                let end = cursor.next_page(1, &mut limits, &|| false).unwrap();
                assert!(end.rows().is_empty());
                assert!(end.reached_end());
            }
            assert_eq!(source.payload_reads.get(), 0);
        }
    }
}

#[test]
fn failed_late_long_value_discards_the_whole_pending_batch_and_snapshot_suffix() {
    for code in [1, 2, 5, 6] {
        for fault in [
            PayloadFault::WrongOwner,
            PayloadFault::ShortChain,
            PayloadFault::Cycle,
            PayloadFault::MissingPage,
        ] {
            let cancelled = Cell::new(false);
            let mut source = SparsePayloads::new(code, &cancelled);
            match fault {
                PayloadFault::WrongOwner => {
                    source.pages.get_mut(&FINAL_MEMO).unwrap()[PAGE_OWNER..PAGE_OWNER + 4]
                        .copy_from_slice(b"BAD!");
                }
                PayloadFault::ShortChain => {
                    source
                        .pages
                        .insert(FINAL_MEMO, payload_page(&[0, 0, 0, 0, 0xff]));
                }
                PayloadFault::Cycle => {
                    source.pages.insert(
                        FINAL_MEMO,
                        payload_page(
                            &[(FINAL_MEMO << 8).to_le_bytes().as_slice(), &[0xff]].concat(),
                        ),
                    );
                }
                PayloadFault::MissingPage => {
                    source.pages.remove(&FINAL_MEMO);
                }
            }
            let original = (source.prefix.clone(), source.pages.clone());
            let reads = Rc::clone(&source.reads);
            let mut limits = budget();
            {
                let mut cursor =
                    TableCursor::open(&mut source, SCOPE, "Items", &mut limits, &|| false).unwrap();
                assert_eq!(
                    cursor.next_page(1, &mut limits, &|| false).unwrap().rows(),
                    [expected_payloads(1)]
                );
                let expected_error = if matches!(fault, PayloadFault::MissingPage) {
                    InspectError::Io(io::ErrorKind::PermissionDenied)
                } else {
                    InspectError::Malformed
                };
                assert_eq!(
                    cursor.next_page(2, &mut limits, &|| false).err(),
                    Some(expected_error),
                    "profile={code}, fault={fault:?}"
                );
                assert_eq!(cursor.position(), 1);
                assert!(cursor.is_failed());
                let read_count = reads.get();
                let remaining = limits.remaining(Resource::Steps);
                assert!(matches!(
                    cursor.next_page(1, &mut limits, &|| false),
                    Err(InspectError::CursorFailed)
                ));
                assert!(matches!(
                    cursor.into_snapshot(TableSnapshotLimits::default(), &mut limits, &|| false),
                    Err(InspectError::CursorFailed)
                ));
                assert_eq!(reads.get(), read_count);
                assert_eq!(limits.remaining(Resource::Steps), remaining);
            }
            assert_eq!((source.prefix, source.pages), original);
        }
    }
}

#[test]
fn cancellation_after_high_payload_decode_closes_continuation_without_publishing() {
    for code in [1, 2, 5, 6] {
        let cancelled = Cell::new(false);
        let mut source = SparsePayloads::new(code, &cancelled);
        source.cancel_page = Some(FINAL_MEMO);
        let mut limits = budget();
        let mut cursor = TableCursor::open(&mut source, SCOPE, "Items", &mut limits, &|| {
            cancelled.get()
        })
        .unwrap();
        assert_eq!(
            cursor
                .next_page(1, &mut limits, &|| cancelled.get())
                .unwrap()
                .rows(),
            [expected_payloads(1)]
        );
        assert!(matches!(
            cursor.next_page(2, &mut limits, &|| cancelled.get()),
            Err(InspectError::Cancelled)
        ));
        assert_eq!(cursor.position(), 1);
        assert!(cursor.is_failed());
    }
}

#[test]
fn external_payload_work_retains_cumulative_resource_limits_after_a_good_prefix() {
    for code in [1, 2, 5, 6] {
        for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
            let cancelled = Cell::new(false);
            let mut source = SparsePayloads::new(code, &cancelled);
            let mut limits = budget();
            let mut cursor =
                TableCursor::open(&mut source, SCOPE, "Items", &mut limits, &|| false).unwrap();
            cursor.next_page(1, &mut limits, &|| false).unwrap();
            limits.charge(resource, limits.remaining(resource)).unwrap();
            assert!(matches!(
                cursor.next_page(2, &mut limits, &|| false),
                Err(InspectError::Limit(_))
            ));
            assert_eq!(cursor.position(), 1);
            assert!(cursor.is_failed());
        }
    }
}
