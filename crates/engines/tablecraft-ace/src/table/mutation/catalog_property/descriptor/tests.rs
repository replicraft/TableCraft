//! Raw object roles and forwarded roots, independent of descriptor producers.
use super::*;
use crate::{
    NativeImage,
    format::row::reference,
    table::mutation::catalog_property::{catalog, column},
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::{self as number, mixed as raw},
    canonical_creation, canonical_database_options, canonical_native17_creation,
};

const SCOPE: CatalogScope = CatalogScope::new(948);
const USER_NATIVE_ID: i32 = 7;
const DATABASE_NATIVE_ID: i32 = 0x0f00_0004;
const PAGE_BYTES: usize = 4096;
const CATALOG_PAGE: usize = 4;
const SLOT_DIRECTORY: usize = 14;
const HIDDEN_SLOT: usize = 8;
const OFFSET_MASK: usize = 0x1fff;
const RESERVED_AT: usize = 8;

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 20_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn observe(image: &NativeImage, native_id: i32) -> Descriptor {
    let schema = catalog(image, &mut budget(), &|| false).unwrap();
    let column = column(&schema).unwrap();
    let origin = image
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.native_id() == native_id)
        .unwrap();
    let mut input = image.reader();
    let mut work = budget();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget: &mut work,
        cancelled: &|| false,
        reads: 0,
    };
    capture(&mut reader, &schema, column, origin).unwrap()
}

#[test]
fn absent_inline_and_forwarded_user_records_keep_actual_carrier_behavior() {
    let absent = number::source(number::Source::IdOnly);
    let descriptor = observe(&image(&absent), USER_NATIVE_ID);
    assert!(descriptor.range.is_none());
    assert!(descriptor.bytes().is_empty());
    assert!(!descriptor.external().unwrap());

    let inline = number::source(number::Source::GuiRequiredZero);
    let descriptor = observe(&image(&inline), USER_NATIVE_ID);
    assert!(descriptor.range.is_some());
    assert_eq!(descriptor.bytes()[3], 0x80);
    assert!(!descriptor.external().unwrap());

    let external = raw::append_text(&raw::source(raw::SourceCase::IdOnly), "Label", 255, true);
    let native = image(&external);
    let user = native
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.native_id() == USER_NATIVE_ID)
        .unwrap();
    assert_eq!(user.slot(), 7);
    let descriptor = observe(&native, USER_NATIVE_ID);
    assert!(descriptor.external().unwrap());
    assert!(descriptor.canonical_single(reference(10, 0), 100).unwrap());
    let catalog = &external[CATALOG_PAGE * PAGE_BYTES..(CATALOG_PAGE + 1) * PAGE_BYTES];
    let start = usize::from(u16::from_le_bytes(
        catalog[SLOT_DIRECTORY + HIDDEN_SLOT * 2..SLOT_DIRECTORY + HIDDEN_SLOT * 2 + 2]
            .try_into()
            .unwrap(),
    )) & OFFSET_MASK;
    assert_eq!(descriptor.record.offset, CATALOG_PAGE * PAGE_BYTES + start);
}

#[test]
fn database_role_uses_its_real_origin_for_keyless_and_primary_property_pages() {
    let keyless = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::Empty).bytes,
    );
    let primary = number::source(number::Source::IdOnly);
    for (before, page) in [(keyless, 9_u32), (primary, 10_u32)] {
        let after = canonical_database_options::explicit_off(&before);
        let native = image(&after);
        let db = native
            .catalog()
            .origins()
            .iter()
            .find(|origin| origin.native_id() == DATABASE_NATIVE_ID)
            .unwrap();
        assert_eq!(db.type_code(), 2);
        assert_eq!(db.parent_id(), 0x0f00_0002);
        let database = observe(&native, DATABASE_NATIVE_ID);
        assert!(database.canonical_single(reference(page, 0), 152).unwrap());
        assert!(database.external().unwrap());
        let user = observe(&native, USER_NATIVE_ID);
        assert!(!user.external().unwrap());
        assert!(!user.canonical_single(reference(page, 0), 152).unwrap());
    }
}

#[test]
fn strict_single_binding_does_not_tighten_generic_carrier_inspection() {
    let after = raw::append_text(&raw::source(raw::SourceCase::IdOnly), "Label", 255, true);
    let mut descriptor = observe(&image(&after), USER_NATIVE_ID);
    assert!(descriptor.canonical_single(reference(10, 0), 100).unwrap());
    assert!(!descriptor.canonical_single(reference(10, 1), 100).unwrap());
    assert!(!descriptor.canonical_single(reference(10, 0), 152).unwrap());
    let start = descriptor.range.as_ref().unwrap().start;
    descriptor.record.data[start + RESERVED_AT] = 1;
    assert!(
        descriptor.external().unwrap(),
        "generic reads preserve unknown reserved bytes"
    );
    assert!(!descriptor.canonical_single(reference(10, 0), 100).unwrap());
}

#[test]
fn native_single_header_goldens_have_closed_reference_length_and_reserved_fields() {
    let valid = [152, 0, 0, 64, 0, 10, 0, 0, 0, 0, 0, 0];
    assert!(trace::canonical_single(&valid, 0x0000_0a00, 152).unwrap());
    for at in [0, 3, 4, 8] {
        let mut changed = valid;
        changed[at] ^= 1;
        assert!(!trace::canonical_single(&changed, 0x0000_0a00, 152).unwrap());
    }
    for bytes in [
        &[][..],
        &valid[..11],
        &[152, 0, 0, 128, 0, 10, 0, 0, 0, 0, 0, 0][..],
    ] {
        assert!(!trace::canonical_single(bytes, 0x0000_0a00, 152).unwrap());
    }
}

#[test]
fn descriptor_capture_preserves_type_owner_cancellation_and_budget_errors() {
    let native = image(&raw::source(raw::SourceCase::IdOnly));
    let schema = catalog(&native, &mut budget(), &|| false).unwrap();
    let property = column(&schema).unwrap();
    let origin = native
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.native_id() == USER_NATIVE_ID)
        .unwrap();
    for cancelled in [false, true] {
        let mut input = native.reader();
        let mut work = if cancelled {
            budget()
        } else {
            ResourceBudget::new(0, 0, 0)
        };
        let callback = || cancelled;
        let mut reader = Reader {
            source: &mut input,
            header: native.catalog().header(),
            budget: &mut work,
            cancelled: &callback,
            reads: 0,
        };
        assert!(capture(&mut reader, &schema, property, origin).is_err());
    }
    let mut input = native.reader();
    let mut work = budget();
    let mut reader = Reader {
        source: &mut input,
        header: native.catalog().header(),
        budget: &mut work,
        cancelled: &|| false,
        reads: 0,
    };
    assert!(matches!(
        capture(&mut reader, &schema, 0, origin),
        Err(InspectError::Unsupported)
    ));
    assert!(matches!(
        capture(&mut reader, &schema, schema.columns().len(), origin),
        Err(InspectError::Malformed)
    ));
}
