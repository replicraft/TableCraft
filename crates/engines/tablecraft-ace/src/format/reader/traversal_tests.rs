use super::Reader;
use crate::{Header, InspectError};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::ResourceBudget;

fn source(last: usize) -> Vec<u8> {
    let mut bytes = vec![0; (last + 1) * 4096];
    bytes[..4].copy_from_slice(&[0, 1, 0, 0]);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 2;
    bytes[62..66].copy_from_slice(&[0xfb, 0x8a, 0xbc, 0x4e]);
    for page in 1..=last {
        let start = page * 4096;
        bytes[start..start + 2].copy_from_slice(&[1, 1]);
        bytes[start + 4..start + 8].copy_from_slice(&9_u32.to_le_bytes());
        bytes[start + 12..start + 14].copy_from_slice(&1_u16.to_le_bytes());
        let flags = if page == last { 0x8000 } else { 0x4000 };
        bytes[start + 14..start + 16].copy_from_slice(&(4092_u16 | flags).to_le_bytes());
        let value = if page == last {
            0xefbe_adde
        } else {
            ((page + 1) as u32) << 8
        };
        bytes[start + 4092..start + 4096].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn allowance() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 0, 1_000_000)
}

fn reader<'a>(
    source: &'a mut Cursor<&'a [u8]>,
    budget: &'a mut ResourceBudget,
    cancelled: &'a dyn Fn() -> bool,
) -> Reader<'a, Cursor<&'a [u8]>> {
    Reader {
        header: Header::parse(source.get_ref(), source.get_ref().len() as u64).unwrap(),
        source,
        budget,
        cancelled,
        reads: 0,
    }
}

#[test]
fn reads_more_than_sixteen_forwarding_hops_but_keeps_physical_proof_scope() {
    let bytes = source(35);
    let mut cursor = Cursor::new(bytes.as_slice());
    let mut budget = allowance();
    let mut reader = reader(&mut cursor, &mut budget, &|| false);
    assert_eq!(reader.row(1, 0, Some(9)).unwrap(), [0xde, 0xad, 0xbe, 0xef]);
    let root = bytes[4096..8192].try_into().unwrap();
    assert_eq!(
        reader.row_from_page(1, 0, 9, root).unwrap(),
        [0xde, 0xad, 0xbe, 0xef]
    );
    let location = reader.read_location(1, 0, 9, root).unwrap();
    assert_eq!(location.data, [0xde, 0xad, 0xbe, 0xef]);
    assert_eq!(location.offset, 35 * 4096 + 4092);
    assert!(matches!(
        reader.row_location(1, 0, 9, root),
        Err(InspectError::StructureLimit)
    ));
}

#[test]
fn repeated_page_reads_are_governed_by_source_geometry_and_explicit_allowances() {
    let bytes = source(2);
    let mut cursor = Cursor::new(bytes.as_slice());
    let mut budget = allowance();
    let mut reader = reader(&mut cursor, &mut budget, &|| false);
    for _ in 0..4100 {
        reader.page(1).unwrap();
    }
    assert_eq!(reader.reads, 4100);
    assert!(matches!(reader.page(3), Err(InspectError::Malformed)));
}

#[test]
fn long_forwarded_reads_reject_cycles_foreign_owners_and_invalid_target_slots() {
    for mutation in 0..3 {
        let mut bytes = source(35);
        match mutation {
            0 => {
                let terminal = 35 * 4096;
                bytes[terminal + 14..terminal + 16].copy_from_slice(&0x4ffc_u16.to_le_bytes());
                bytes[terminal + 4092..terminal + 4096].copy_from_slice(&0x0a00_u32.to_le_bytes());
            }
            1 => bytes[20 * 4096 + 4..20 * 4096 + 8].copy_from_slice(&10_u32.to_le_bytes()),
            _ => bytes[20 * 4096 + 4092..21 * 4096].copy_from_slice(&0x1501_u32.to_le_bytes()),
        }
        let mut cursor = Cursor::new(bytes.as_slice());
        let mut budget = allowance();
        assert!(matches!(
            reader(&mut cursor, &mut budget, &|| false).row(1, 0, Some(9)),
            Err(InspectError::Malformed)
        ));
    }
}

#[test]
fn long_forwarded_reads_observe_cancellation_and_explicit_work_exhaustion() {
    let bytes = source(35);
    let calls = Cell::new(0);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() == 80
    };
    let mut cursor = Cursor::new(bytes.as_slice());
    let mut budget = allowance();
    assert!(matches!(
        reader(&mut cursor, &mut budget, &cancelled).row(1, 0, Some(9)),
        Err(InspectError::Cancelled)
    ));

    let mut cursor = Cursor::new(bytes.as_slice());
    let mut budget = ResourceBudget::new(64 * 1024 * 1024, 0, 10);
    assert!(matches!(
        reader(&mut cursor, &mut budget, &|| false).row(1, 0, Some(9)),
        Err(InspectError::Limit(_))
    ));
}
