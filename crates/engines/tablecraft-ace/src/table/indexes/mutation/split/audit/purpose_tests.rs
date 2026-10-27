//! Private counterfactual reader proof, not an append-allocator certificate.
use crate::{Header, InspectError, format::reader::Reader, table::long_value};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

const PAGE_BYTES: usize = 4096;
const FUTURE_PAGE: usize = 3;
const OWNER: u32 = 10;
const PAYLOAD: u8 = 0x71;
const NON_DATA_TAGS: [[u8; 2]; 3] = [[3, 1], [4, 1], [5, 1]];

fn image() -> Vec<u8> {
    let mut bytes = vec![0; FUTURE_PAGE * PAGE_BYTES];
    bytes[..4].copy_from_slice(&[0, 1, 0, 0]);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = 6;
    bytes[62..66].copy_from_slice(&[0xfb, 0x8a, 0xbc, 0x4e]);
    bytes
}

fn row_page(owner: [u8; 4], payload: &[u8], flags: u16) -> Vec<u8> {
    let mut page = vec![0; PAGE_BYTES];
    page[..2].copy_from_slice(&[1, 1]);
    page[4..8].copy_from_slice(&owner);
    page[12..14].copy_from_slice(&1_u16.to_le_bytes());
    let start = PAGE_BYTES - payload.len();
    page[14..16].copy_from_slice(&(start as u16 | flags).to_le_bytes());
    page[start..].copy_from_slice(payload);
    page
}

fn ordinary(bytes: &[u8], page: u32) -> Result<Vec<u8>, InspectError> {
    let header = Header::parse(&bytes[..66], bytes.len() as u64).unwrap();
    let mut source = Cursor::new(bytes);
    Reader {
        source: &mut source,
        header,
        budget: &mut ResourceBudget::new(1_048_576, 100, 100_000),
        cancelled: &|| false,
        reads: 0,
    }
    .row(page, 0, Some(OWNER))
}

fn lval(bytes: &[u8], reference: u32, tagged_length: u32) -> Result<Option<Vec<u8>>, InspectError> {
    let header = Header::parse(&bytes[..66], bytes.len() as u64).unwrap();
    let mut source = Cursor::new(bytes);
    let descriptor = [tagged_length.to_le_bytes(), reference.to_le_bytes(), [0; 4]].concat();
    long_value::read(
        &mut Reader {
            source: &mut source,
            header,
            budget: &mut ResourceBudget::new(1_048_576, 100, 100_000),
            cancelled: &|| false,
            reads: 0,
        },
        &descriptor,
    )
}

#[test]
fn a_future_direct_or_forwarded_row_needs_a_data_target_even_with_valid_slot_bytes() {
    let mut original = image();
    let target = (FUTURE_PAGE as u32) << 8;
    original[PAGE_BYTES..2 * PAGE_BYTES].copy_from_slice(&row_page(
        OWNER.to_le_bytes(),
        &target.to_le_bytes(),
        0x4000,
    ));
    assert!(ordinary(&original, FUTURE_PAGE as u32).is_err());
    assert!(ordinary(&original, 1).is_err());

    let target_page = row_page(OWNER.to_le_bytes(), &[PAYLOAD], 0);
    let mut data = original.clone();
    data.extend_from_slice(&target_page);
    assert_eq!(ordinary(&data, FUTURE_PAGE as u32).unwrap(), [PAYLOAD]);
    assert_eq!(ordinary(&data, 1).unwrap(), [PAYLOAD]);

    for tag in NON_DATA_TAGS {
        let mut candidate = data.clone();
        candidate[FUTURE_PAGE * PAGE_BYTES..FUTURE_PAGE * PAGE_BYTES + 2].copy_from_slice(&tag);
        for page in [1, FUTURE_PAGE as u32] {
            assert_eq!(ordinary(&candidate, page), Err(InspectError::Malformed));
        }
    }
}

#[test]
fn single_and_chained_lval_references_do_not_decode_non_data_append_pages() {
    let target = (FUTURE_PAGE as u32) << 8;
    let original = image();
    assert!(lval(&original, target, 0x4000_0001).is_err());
    let mut data = original.clone();
    data.extend_from_slice(&row_page(*b"LVAL", &[PAYLOAD], 0));
    assert_eq!(
        lval(&data, target, 0x4000_0001).unwrap(),
        Some(vec![PAYLOAD])
    );
    for tag in NON_DATA_TAGS {
        let mut candidate = data.clone();
        candidate[FUTURE_PAGE * PAGE_BYTES..FUTURE_PAGE * PAGE_BYTES + 2].copy_from_slice(&tag);
        assert_eq!(
            lval(&candidate, target, 0x4000_0001),
            Err(InspectError::Malformed)
        );
    }

    let future = ((FUTURE_PAGE + 1) as u32) << 8;
    let first_chunk = [future.to_le_bytes().as_slice(), &[PAYLOAD]].concat();
    let mut chained = original;
    chained.extend_from_slice(&row_page(*b"LVAL", &first_chunk, 0));
    assert!(lval(&chained, target, 2).is_err());
    let last_chunk = [0_u32.to_le_bytes().as_slice(), &[PAYLOAD]].concat();
    chained.extend_from_slice(&row_page(*b"LVAL", &last_chunk, 0));
    assert_eq!(lval(&chained, target, 2).unwrap(), Some(vec![PAYLOAD; 2]));
    for tag in NON_DATA_TAGS {
        let mut candidate = chained.clone();
        candidate[(FUTURE_PAGE + 1) * PAGE_BYTES..(FUTURE_PAGE + 1) * PAGE_BYTES + 2]
            .copy_from_slice(&tag);
        assert_eq!(lval(&candidate, target, 2), Err(InspectError::Malformed));
    }
}
