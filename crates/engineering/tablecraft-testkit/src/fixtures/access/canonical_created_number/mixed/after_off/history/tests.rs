//! Literal SDK pins and role-bound chronology remain independent of native code.
use super::super::{append_number, append_text};
use super::{SourceCase, source};
use crate::fixtures::access::canonical_created_number::{DIRECTORY, PAGE, word};
#[cfg(any(feature = "access-support", feature = "cli-support"))]
use sha2::{Digest, Sha256};

const CATALOG_PAGE: usize = 4;
const PROPERTY_PAGE: usize = 10;
const MAP_PAGE: usize = 3;
const INDEX_PAGE: usize = 9;
const COUNT_AT: usize = 12;
const OFFSET_MASK: usize = 0x1fff;
const SLOT_BYTES: usize = size_of::<u16>();
const DB_ID_AT: usize = 2;
const DB_PARENT_AT: usize = 6;
const DB_TYPE_AT: usize = 10;
const DB_FLAGS_AT: usize = 28;
const DB_PROPERTY_AT: usize = 44;
const DB_REFERENCE_AT: usize = DB_PROPERTY_AT + size_of::<u32>();
const DB_BODY_BYTES: usize = 152;

#[cfg(any(feature = "access-support", feature = "cli-support"))]
fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(any(feature = "access-support", feature = "cli-support"))]
#[test]
fn two_before_and_four_complete_afters_match_frozen_second_add_sdk_images() {
    for (case, before_pin, number_pin, text_pin) in [
        (
            SourceCase::NumberThenNumber,
            "fbcc7a8ab6fa4d10df64c376c80b99ca76d1200d64d8d4a5d31d7b3702cc7680",
            "cf07d4ba065af7f694c263b3344a01ad6fd6a8f4ebe1e073088626ed2541919f",
            "020379bbab66648cb5f2b580f8cc5e903132d451483066dea94296bb967674b7",
        ),
        (
            SourceCase::MixedThenText,
            "d5940a703ae582f27958b4369c8cdca97d9f566dd7cb79c63c4a39e72edf4002",
            "a28268a3acb88767749b3d640e1dc2b072ff0fda515ccfc18735be113c337bd3",
            "275af21a3140ab69a320b7b2a86ef79207443d67777d55381f742d0cd6dab04a",
        ),
    ] {
        let before = source(case);
        assert_eq!(hash(&before), before_pin);
        assert_eq!(hash(&append_number(&before, "SecondNumber")), number_pin);
        assert_eq!(hash(&append_text(&before, "SecondLabel")), text_pin);
    }
}

fn long(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + size_of::<u32>()].try_into().unwrap())
}

fn row(page: &[u8], slot: usize) -> (usize, usize) {
    let start = word(page, DIRECTORY + slot * SLOT_BYTES) & OFFSET_MASK;
    let end = if slot == 0 {
        PAGE
    } else {
        word(page, DIRECTORY + (slot - 1) * SLOT_BYTES) & OFFSET_MASK
    };
    (start, end)
}

#[test]
fn actual_db_reference_and_all_retired_rows_remain_fixed_after_second_append() {
    for (case, db_row, user_row) in [
        (SourceCase::NumberThenNumber, 1, 2),
        (SourceCase::MixedThenText, 3, 4),
    ] {
        let before = source(case);
        let catalog = &before[CATALOG_PAGE * PAGE..(CATALOG_PAGE + 1) * PAGE];
        let db_slot = word(catalog, COUNT_AT) - 1;
        let (db_start, db_end) = row(catalog, db_slot);
        let record = &catalog[db_start..db_end];
        assert_eq!(long(record, DB_ID_AT), 0x0f000004);
        assert_eq!(long(record, DB_PARENT_AT), 0x0f000002);
        assert_eq!(word(record, DB_TYPE_AT), 2);
        assert_eq!(long(record, DB_FLAGS_AT), 0x80000000);
        assert_eq!(long(record, DB_PROPERTY_AT), 0x40000098);
        assert_eq!(
            long(record, DB_REFERENCE_AT),
            ((PROPERTY_PAGE as u32) << 8) | db_row as u32
        );
        let old = &before[PROPERTY_PAGE * PAGE..(PROPERTY_PAGE + 1) * PAGE];
        let count = word(old, COUNT_AT);
        assert_eq!(count, user_row + 1);
        assert!(db_row < user_row);
        let (body_start, body_end) = row(old, db_row);
        assert_eq!(body_end - body_start, DB_BODY_BYTES);
        for after in [
            append_number(&before, "SecondNumber"),
            append_text(&before, "SecondLabel"),
        ] {
            assert_eq!(after.len(), before.len());
            assert_eq!(
                &after[CATALOG_PAGE * PAGE + db_start..CATALOG_PAGE * PAGE + db_end],
                record
            );
            for page in [MAP_PAGE, INDEX_PAGE] {
                assert_eq!(
                    &after[page * PAGE..(page + 1) * PAGE],
                    &before[page * PAGE..(page + 1) * PAGE]
                );
            }
            let new = &after[PROPERTY_PAGE * PAGE..(PROPERTY_PAGE + 1) * PAGE];
            assert_eq!(word(new, COUNT_AT), count + 1);
            for slot in 0..count {
                let (start, end) = row(old, slot);
                assert_eq!(&new[start..end], &old[start..end]);
                let entry = DIRECTORY + slot * SLOT_BYTES;
                assert_eq!(
                    &new[entry..entry + SLOT_BYTES],
                    &old[entry..entry + SLOT_BYTES]
                );
            }
        }
    }
}
