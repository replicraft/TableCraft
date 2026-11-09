//! Independent raw roles, forwarding and current narrow history policies.
use super::*;
use crate::NativeImage;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::{self as number, mixed as raw},
    canonical_creation, canonical_database_options, canonical_native17_creation,
};

const SCOPE: CatalogScope = CatalogScope::new(963);
const USER_NATIVE_ID: i32 = 7;
const DATABASE_NATIVE_ID: i32 = 0x0f00_0004;
const PAGE_BYTES: usize = 4096;
const CATALOG_PAGE: usize = 4;
const SLOT_DIRECTORY: usize = 14;
const HIDDEN_SLOT: usize = 8;
const RAW_OFFSET_MASK: u16 = 0x1fff;

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 20_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn object(image: &NativeImage, id: i32) -> ObjectId {
    image
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.native_id() == id)
        .unwrap()
        .object_id()
}

#[test]
fn user_live_reference_is_separate_from_all_historical_rows_and_forwarded_root() {
    let first = raw::append_text(&raw::source(raw::SourceCase::IdOnly), "Label", 255, true);
    let second = raw::append_number(&first, "Score");
    let third = raw::append_text(&second, "OtherLabel", 80, true);
    for (bytes, slot) in [(first, 0), (second, 1), (third, 2)] {
        let native = image(&bytes);
        let ledger = capture(&bytes, native.catalog(), &mut budget(), &|| false).unwrap();
        assert_eq!(ledger.pages, [10]);
        let user = object(&native, USER_NATIVE_ID);
        assert!(
            ledger
                .sole_user_history(user, &[10], 10, slot, &mut budget(), &|| false)
                .unwrap()
        );
        assert!(
            !ledger
                .sole_user_history(user, &[10], 10, slot + 1, &mut budget(), &|| false)
                .unwrap()
        );
        let live = ledger
            .live
            .iter()
            .find(|entry| entry.origin.object_id() == user)
            .unwrap();
        assert_eq!(live.origin.slot(), 7);
        assert_eq!(live.chunks.len(), 1);
        assert_eq!(live.chunks[0].reference, (10 << 8) | u32::from(slot));
        // The catalog logical root7 follows the hidden physical slot8.
        let word_at = CATALOG_PAGE * PAGE_BYTES + SLOT_DIRECTORY + HIDDEN_SLOT * 2;
        let word = u16::from_le_bytes(bytes[word_at..word_at + 2].try_into().unwrap());
        assert_eq!(
            live.descriptor.physical_offset(),
            CATALOG_PAGE * PAGE_BYTES + usize::from(word & RAW_OFFSET_MASK)
        );
    }
}

#[test]
fn database_role_and_keyless_page9_do_not_become_user_history() {
    let keyless = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::Empty).bytes,
    );
    let primary = number::source(number::Source::IdOnly);
    for (before, page) in [(keyless, 9), (primary, 10)] {
        let after = canonical_database_options::explicit_off(&before);
        let native = image(&after);
        let ledger = capture(&after, native.catalog(), &mut budget(), &|| false).unwrap();
        assert_eq!(ledger.pages, [page]);
        let database = object(&native, DATABASE_NATIVE_ID);
        assert!(
            ledger
                .sole_single(database, page << 8, 152, &mut budget(), &|| false)
                .unwrap()
        );
        let user = object(&native, USER_NATIVE_ID);
        assert!(
            !ledger
                .sole_user_history(user, &[page], page, 0, &mut budget(), &|| false)
                .unwrap()
        );
    }
}

#[test]
fn absent_and_inline_objects_do_not_imply_external_column_rows() {
    for source in [number::Source::IdOnly, number::Source::GuiRequiredZero] {
        let bytes = number::source(source);
        let native = image(&bytes);
        let ledger = capture(&bytes, native.catalog(), &mut budget(), &|| false).unwrap();
        assert!(ledger.pages.is_empty());
        assert!(ledger.live.iter().all(|entry| entry.chunks.is_empty()));
        let user = object(&native, USER_NATIVE_ID);
        assert!(
            !ledger
                .sole_user_history(user, &[], 10, 0, &mut budget(), &|| false)
                .unwrap()
        );
    }
}

#[test]
fn supplied_catalog_is_bound_to_full_header_length_and_actual_record_identity() {
    let bytes = raw::append_text(&raw::source(raw::SourceCase::IdOnly), "Label", 255, true);
    let native = image(&bytes);
    let ledger = capture(&bytes, native.catalog(), &mut budget(), &|| false).unwrap();
    let user = ledger
        .live
        .iter()
        .find(|entry| entry.origin.native_id() == USER_NATIVE_ID)
        .unwrap();
    let mut changed = bytes.clone();
    // Independent raw MSysObjects fixed Id field: column-count word then Int32.
    let id_at = user.descriptor.physical_offset() + 2;
    changed[id_at..id_at + 4].copy_from_slice(&8_i32.to_le_bytes());
    assert!(matches!(
        capture(&changed, native.catalog(), &mut budget(), &|| false),
        Err(EditError::Inspect(InspectError::Malformed))
    ));
    changed = bytes.clone();
    changed.resize(changed.len() + 4096, 0);
    assert!(matches!(
        capture(&changed, native.catalog(), &mut budget(), &|| false),
        Err(EditError::Inspect(InspectError::Malformed))
    ));
}

#[test]
fn observation_and_policy_keep_cancellation_and_cumulative_limits() {
    let bytes = raw::append_number(&raw::source(raw::SourceCase::IdOnly), "Score");
    let native = image(&bytes);
    assert!(matches!(
        capture(&bytes, native.catalog(), &mut budget(), &|| true),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    let mut empty = ResourceBudget::new(0, 0, 0);
    assert!(capture(&bytes, native.catalog(), &mut empty, &|| false).is_err());
    let ledger = capture(&bytes, native.catalog(), &mut budget(), &|| false).unwrap();
    assert!(matches!(
        ledger.sole_user_history(
            object(&native, USER_NATIVE_ID),
            &[10],
            10,
            0,
            &mut budget(),
            &|| true
        ),
        Err(InspectError::Cancelled)
    ));
}

#[test]
fn shared_column_has_two_distinct_live_owners_without_reclassifying_database_as_add() {
    let before = number::repeated::source(number::Source::IdOnly);
    let after = canonical_database_options::append_explicit_off_to_owned_page(&before);
    let native = image(&after);
    let ledger = capture(&after, native.catalog(), &mut budget(), &|| false).unwrap();
    assert_eq!(ledger.pages, [10]);
    let user = object(&native, USER_NATIVE_ID);
    let database = object(&native, DATABASE_NATIVE_ID);
    let user_live = ledger
        .live
        .iter()
        .find(|entry| entry.origin.object_id() == user)
        .unwrap();
    let database_live = ledger
        .live
        .iter()
        .find(|entry| entry.origin.object_id() == database)
        .unwrap();
    assert_eq!(user_live.chunks.len(), 1);
    assert_eq!(user_live.chunks[0].reference, 10 << 8);
    assert_eq!(database_live.chunks.len(), 1);
    assert_eq!(database_live.chunks[0].reference, (10 << 8) | 1);
    assert_eq!(database_live.bytes, 152);
    assert_eq!(user_live.origin.slot(), 7);
    assert_eq!(database_live.origin.slot(), 9);
    assert!(
        !ledger
            .sole_user_history(user, &[10], 10, 1, &mut budget(), &|| false)
            .unwrap()
    );
    assert!(
        !ledger
            .sole_single(user, 10 << 8, user_live.bytes, &mut budget(), &|| false)
            .unwrap()
    );
    assert!(
        !ledger
            .sole_single(database, (10 << 8) | 1, 152, &mut budget(), &|| false)
            .unwrap()
    );
}
