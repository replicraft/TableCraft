use super::super::storage;
use super::{SourceCase, append_number, append_text, source};
#[cfg(any(feature = "access-support", feature = "cli-support"))]
use sha2::{Digest, Sha256};

const PAGE: usize = 4096;
const DIRECTORY: usize = 14;
const COUNT: usize = 12;
const OFFSET_MASK: usize = 0x1fff;

fn word(bytes: &[u8], offset: usize) -> usize {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()) as usize
}

#[cfg(any(feature = "access-support", feature = "cli-support"))]
fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(any(feature = "access-support", feature = "cli-support"))]
#[test]
fn two_sources_and_four_complete_afters_match_independent_sdk_images() {
    for (case, before_pin, number_pin, text_pin) in [
        (
            SourceCase::Number,
            "c88bc4cb1090d9b0c5e011010ec76cf9acc33da7ec9d58e67fa873bfaabe18ef",
            "fbcc7a8ab6fa4d10df64c376c80b99ca76d1200d64d8d4a5d31d7b3702cc7680",
            "0ebbef86bf96f5079a2b666ab375f0da2d220322d3feb124f7c94e17d6280240",
        ),
        (
            SourceCase::Mixed,
            "c28d3c1bf1de4e75f894ac80ae332aa2a5b4b8c3c9111700d989222b8d6267c3",
            "9011e9cbe157a529a5c7e362d6555abc6f13c68179ab2e2cb59ddb2d36c15e66",
            "d5940a703ae582f27958b4369c8cdca97d9f566dd7cb79c63c4a39e72edf4002",
        ),
    ] {
        let before = source(case);
        assert_eq!(hash(&before), before_pin, "{case:?} BEFORE");
        assert_eq!(
            hash(&append_number(&before, "AfterOff")),
            number_pin,
            "{case:?} Number"
        );
        assert_eq!(
            hash(&append_text(&before, "LabelOff")),
            text_pin,
            "{case:?} Text"
        );
    }
}

#[test]
fn database_body_record_and_all_old_rows_survive_both_additions() {
    for case in [SourceCase::Number, SourceCase::Mixed] {
        let before = source(case);
        let old = &before[10 * PAGE..11 * PAGE];
        let count = word(old, COUNT);
        let database_slot = count - 1;
        let database_start = word(old, DIRECTORY + database_slot * 2) & OFFSET_MASK;
        let database_end = word(old, DIRECTORY + (database_slot - 1) * 2) & OFFSET_MASK;
        assert_eq!(database_end - database_start, 152);
        // The real user descriptor selects the preceding body, not the DB ROW.
        let user_body = storage::selected_body(&before);
        assert_ne!(user_body.len(), 152);
        let catalog = &before[4 * PAGE..5 * PAGE];
        let database_catalog_slot = word(catalog, COUNT) - 1;
        let record_start = word(catalog, DIRECTORY + database_catalog_slot * 2) & OFFSET_MASK;
        let record_end = word(catalog, DIRECTORY + (database_catalog_slot - 1) * 2) & OFFSET_MASK;
        for after in [
            append_number(&before, "AfterOff"),
            append_text(&before, "LabelOff"),
        ] {
            assert_eq!(after.len(), before.len());
            assert_eq!(
                &after[4 * PAGE + record_start..4 * PAGE + record_end],
                &catalog[record_start..record_end]
            );
            assert_eq!(&after[3 * PAGE..4 * PAGE], &before[3 * PAGE..4 * PAGE]);
            assert_eq!(&after[9 * PAGE..10 * PAGE], &before[9 * PAGE..10 * PAGE]);
            let new = &after[10 * PAGE..11 * PAGE];
            assert_eq!(word(new, COUNT), count + 1);
            assert_eq!(
                &new[database_start..database_end],
                &old[database_start..database_end]
            );
            for slot in 0..count {
                let entry = DIRECTORY + slot * 2;
                assert_eq!(&new[entry..entry + 2], &old[entry..entry + 2]);
                let start = word(old, entry) & OFFSET_MASK;
                let end = if slot == 0 {
                    PAGE
                } else {
                    word(old, entry - 2) & OFFSET_MASK
                };
                assert_eq!(&new[start..end], &old[start..end]);
            }
        }
    }
}

#[test]
#[should_panic]
fn ordinary_body_reader_keeps_its_latest_user_row_guard() {
    storage::body(&source(SourceCase::Number));
}
