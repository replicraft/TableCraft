#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
use tablecraft_testkit::access as support;

use std::io::{self, Read, Seek, SeekFrom};
use tablecraft_ace::{
    InspectError, NativeImage, TablePageRequest, inspect_header, read_table_page,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;
const FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const LAST_PAGE: u32 = (FILE_BYTES / PAGE_BYTES as u64 - 1) as u32;
const USAGE_PAGE: usize = 11;
const DATA_PAGE: usize = 12;

// Original sparse synthetic image: only the small prefix and final page exist
// in memory. Every requested read is bounded, including holes and boundary EOF.
struct SparseImage {
    prefix: Vec<u8>,
    tail: Vec<u8>,
    length: u64,
    position: u64,
    reads: Vec<(u64, usize)>,
}
impl SparseImage {
    fn new(code: u8) -> Self {
        let mut prefix = fixture::database();
        prefix[20] = code;
        if code != 1 {
            prefix[4..20].copy_from_slice(b"Standard ACE DB\0");
        }
        let tail = prefix[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES].to_vec();
        let mut map = vec![0];
        map.extend_from_slice(&LAST_PAGE.to_le_bytes());
        map.push(1);
        prefix[USAGE_PAGE * PAGE_BYTES..(USAGE_PAGE + 1) * PAGE_BYTES]
            .copy_from_slice(&support::data_page(1, &[(0, map)]));
        Self {
            prefix,
            tail,
            length: FILE_BYTES,
            position: 0,
            reads: Vec::new(),
        }
    }
}
impl tablecraft_ace::NativeReadSource for SparseImage {}
impl Read for SparseImage {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.len() > PAGE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unbounded synthetic read",
            ));
        }
        let count = out
            .len()
            .min(self.length.saturating_sub(self.position) as usize);
        self.reads.push((self.position, count));
        out[..count].fill(0);
        let end = self.position + count as u64;
        for (start, bytes) in [
            (0, self.prefix.as_slice()),
            (
                u64::from(LAST_PAGE) * PAGE_BYTES as u64,
                self.tail.as_slice(),
            ),
        ] {
            let from = self.position.max(start);
            let to = end.min(start + bytes.len() as u64);
            if from < to {
                out[(from - self.position) as usize..(to - self.position) as usize]
                    .copy_from_slice(&bytes[(from - start) as usize..(to - start) as usize]);
            }
        }
        self.position = end;
        Ok(count)
    }
}
impl Seek for SparseImage {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let next = match to {
            SeekFrom::Start(value) => i128::from(value),
            SeekFrom::Current(delta) => i128::from(self.position) + i128::from(delta),
            SeekFrom::End(delta) => i128::from(self.length) + i128::from(delta),
        };
        self.position = next
            .try_into()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid seek"))?;
        Ok(self.position)
    }
}
fn budget() -> ResourceBudget {
    ResourceBudget::new(1024 * 1024, 16_384, 1_000_000)
}
fn page(
    input: &mut SparseImage,
    budget: &mut ResourceBudget,
) -> Result<tablecraft_ace::TablePage, InspectError> {
    read_table_page(
        input,
        CatalogScope::new(1),
        "Items",
        TablePageRequest::new(1, 1).unwrap(),
        budget,
        &|| false,
    )
}
#[test]
fn all_read_profiles_fetch_a_final_page_at_two_gib_under_small_budgets() {
    for code in [1, 2, 5, 6] {
        let mut input = SparseImage::new(code);
        let original = input.prefix.clone();
        let mut budget = budget();
        let result = page(&mut input, &mut budget).unwrap();
        assert_eq!(result.rows(), &[vec![Value::Int32(2), Value::Int32(20)]]);
        assert_eq!(
            inspect_header(&mut input, &mut budget, &|| false)
                .unwrap()
                .page_count(),
            LAST_PAGE + 1
        );
        assert!(
            input
                .reads
                .contains(&(u64::from(LAST_PAGE) * PAGE_BYTES as u64, PAGE_BYTES))
        );
        assert!(input.reads.iter().all(|(_, count)| *count <= PAGE_BYTES));
        assert!(input.reads.iter().map(|(_, count)| count).sum::<usize>() < 128 * 1024);
        assert!(budget.remaining(Resource::Bytes) > 512 * 1024);
        assert_eq!(input.prefix, original);
    }
}
#[test]
fn larger_source_geometry_preserves_limits_cancellation_and_bad_page_rejection() {
    let mut input = SparseImage::new(6);
    assert!(matches!(
        page(&mut input, &mut ResourceBudget::new(66, 100, 100)),
        Err(InspectError::Limit(_))
    ));
    let mut input = SparseImage::new(6);
    assert_eq!(
        read_table_page(
            &mut input,
            CatalogScope::new(1),
            "Items",
            TablePageRequest::new(0, 1).unwrap(),
            &mut budget(),
            &|| true
        )
        .err(),
        Some(InspectError::Cancelled)
    );
    assert!(input.reads.is_empty());
    let mut input = SparseImage::new(6);
    input.tail[0] = 0xff;
    assert_eq!(
        page(&mut input, &mut budget()).err(),
        Some(InspectError::Malformed)
    );
    let mut input = SparseImage::new(6);
    input.length -= PAGE_BYTES as u64;
    assert_eq!(
        page(&mut input, &mut budget()).err(),
        Some(InspectError::Malformed)
    );
    let mut input = SparseImage::new(6);
    input.length += PAGE_BYTES as u64;
    assert_eq!(
        inspect_header(&mut input, &mut budget(), &|| false),
        Err(InspectError::StructureLimit)
    );
}
#[test]
fn complete_image_capture_rejects_large_sources_before_image_allocation_or_bulk_reads() {
    for length in [64_u64 * 1024 * 1024 + 4096, FILE_BYTES] {
        let mut input = SparseImage::new(6);
        input.length = length;
        let mut budget = ResourceBudget::new(u64::MAX, 16_384, 1_000_000);
        assert_eq!(
            NativeImage::capture(&mut input, CatalogScope::new(1), &mut budget, &|| false).err(),
            Some(InspectError::StructureLimit)
        );
        assert_eq!(input.reads, [(0, 66)]);
        assert_eq!(budget.remaining(Resource::Bytes), u64::MAX - 66);
    }
}
