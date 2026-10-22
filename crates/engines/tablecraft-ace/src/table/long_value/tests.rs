use super::read;
use crate::{Header, InspectError, format::reader::Reader};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::ResourceBudget;

const PAGE_BYTES: usize = 4096;

// Independently authored pages: one LVAL row per page, with literal wire offsets.
fn chain(chunks: usize, width: usize, version: u8) -> Vec<u8> {
    let mut bytes = vec![0; (chunks + 2) * PAGE_BYTES];
    bytes[..4].copy_from_slice(&[0, 1, 0, 0]);
    bytes[4..20].copy_from_slice(if version == 1 {
        b"Standard Jet DB\0"
    } else {
        b"Standard ACE DB\0"
    });
    bytes[20] = version;
    bytes[62..66].copy_from_slice(&[0xfb, 0x8a, 0xbc, 0x4e]);
    for position in 0..chunks {
        let page = &mut bytes[(position + 2) * PAGE_BYTES..(position + 3) * PAGE_BYTES];
        page[..2].copy_from_slice(&[1, 1]);
        page[4..8].copy_from_slice(b"LVAL");
        page[12..14].copy_from_slice(&1_u16.to_le_bytes());
        let start = PAGE_BYTES - width - 4;
        page[14..16].copy_from_slice(&(start as u16).to_le_bytes());
        let next = if position + 1 == chunks {
            0
        } else {
            ((position + 3) as u32) << 8
        };
        page[start..start + 4].copy_from_slice(&next.to_le_bytes());
        page[start + 4..].fill(position as u8);
    }
    bytes
}

fn decode(
    bytes: &[u8],
    length: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<Vec<u8>>, InspectError> {
    let header = Header::parse(&bytes[..66], bytes.len() as u64).unwrap();
    let mut source = Cursor::new(bytes);
    let definition = [length.to_le_bytes(), (2_u32 << 8).to_le_bytes(), [0; 4]].concat();
    read(
        &mut Reader {
            source: &mut source,
            header,
            budget,
            cancelled,
            reads: 0,
        },
        &definition,
    )
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 100, 10_000_000)
}

#[test]
fn full_raw_ceiling_survives_long_chains_for_each_qualified_family() {
    for version in [1, 2, 5, 6] {
        let bytes = chain(1024, 2048, version);
        let original = bytes.clone();
        let actual = decode(&bytes, 2_097_152, &mut budget(), &|| false)
            .unwrap()
            .unwrap();
        assert_eq!(actual.len(), 2_097_152);
        for (position, chunk) in actual.as_chunks::<2048>().0.iter().enumerate() {
            assert!(chunk.iter().all(|byte| *byte == position as u8));
        }
        assert_eq!(bytes, original);
    }
}

#[test]
fn late_cycles_foreign_pages_truncation_and_chain_ceiling_publish_nothing() {
    let original = chain(1025, 1, 5);
    assert_eq!(
        decode(&original, 1025, &mut budget(), &|| false).err(),
        Some(InspectError::StructureLimit)
    );
    assert_eq!(
        decode(&original, 2_097_153, &mut budget(), &|| false).err(),
        Some(InspectError::StructureLimit)
    );
    let mut cycle = original.clone();
    let start = 202 * PAGE_BYTES - 5;
    cycle[start..start + 4].copy_from_slice(&(2_u32 << 8).to_le_bytes());
    assert_eq!(
        decode(&cycle, 1025, &mut budget(), &|| false).err(),
        Some(InspectError::Malformed)
    );
    let mut foreign = original.clone();
    foreign[201 * PAGE_BYTES + 4] = 0;
    assert_eq!(
        decode(&foreign, 1025, &mut budget(), &|| false).err(),
        Some(InspectError::Malformed)
    );
    let truncated = chain(200, 1, 5);
    assert_eq!(
        decode(&truncated, 201, &mut budget(), &|| false).err(),
        Some(InspectError::Malformed)
    );
    assert_eq!(
        decode(&truncated, 199, &mut budget(), &|| false).err(),
        Some(InspectError::Malformed)
    );
}

#[test]
fn cancellation_and_work_or_byte_exhaustion_never_return_partial_values() {
    let bytes = chain(1024, 2048, 5);
    for stop in [0, 1, 20, 1000, 2000] {
        let calls = Cell::new(0);
        let cancelled = || {
            let previous = calls.get();
            calls.set(previous + 1);
            previous >= stop
        };
        assert_eq!(
            decode(&bytes, 2_097_152, &mut budget(), &cancelled).err(),
            Some(InspectError::Cancelled)
        );
    }
    for mut limited in [
        ResourceBudget::new(32 * 1024 * 1024, 100, 1000),
        ResourceBudget::new(2_097_152, 100, 10_000_000),
    ] {
        assert!(decode(&bytes, 2_097_152, &mut limited, &|| false).is_err());
    }
}
