#[path = "support/agile.rs"]
mod fixture;
use std::{
    fs,
    io::{Cursor, Read, Seek, SeekFrom},
    path::Path,
};
use tablecraft_ace::{
    AgileSource, InspectError, NativeImage, ReadProfile, TableCursor, inspect_catalog,
    inspect_header, read_table,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 2_000_000)
}
fn original() -> Vec<u8> {
    fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap()
}

#[test]
fn authenticated_sources_decode_native_reads_while_raw_io_and_preservation_retain_ciphertext() {
    let plain = original();
    for (code, profile) in [
        (2, ReadProfile::Ace2007),
        (3, ReadProfile::Ace2010),
        (5, ReadProfile::Ace2016),
        (6, ReadProfile::Ace2019),
    ] {
        let encrypted = fixture::encrypted_image(&plain, code, "public 😀 password");
        let mut allowance = budget();
        let mut source = AgileSource::open(
            Cursor::new(encrypted.clone()),
            Some("public 😀 password"),
            &mut allowance,
            &|| false,
        )
        .unwrap();
        let raw = inspect_header(&mut source, &mut allowance, &|| false).unwrap();
        assert!(raw.encoded_pages());
        assert_eq!(raw.read_profile(), Err(InspectError::Encoded));
        let catalog =
            inspect_catalog(&mut source, CatalogScope::new(1), &mut allowance, &|| false).unwrap();
        assert!(catalog.header().encoded_pages());
        assert_eq!(catalog.header().read_profile(), Ok(profile));
        let mut retagged = plain.clone();
        retagged[20] = code;
        let expected = read_table(
            &mut Cursor::new(retagged),
            CatalogScope::new(2),
            "ROULETTE_TB",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let actual = read_table(
            &mut source,
            CatalogScope::new(1),
            "ROULETTE_TB",
            &mut allowance,
            &|| false,
        )
        .unwrap();
        assert_eq!(actual.rows(), expected.rows());
        let mut cursor = TableCursor::open(
            &mut source,
            CatalogScope::new(1),
            "ROULETTE_TB",
            &mut allowance,
            &|| false,
        )
        .unwrap();
        let first = cursor.next_page(7, &mut allowance, &|| false).unwrap();
        assert_eq!(first.rows(), &expected.rows()[..7]);
        drop(cursor);
        assert!(matches!(
            NativeImage::capture(&mut source, CatalogScope::new(1), &mut allowance, &|| false),
            Err(InspectError::Encoded)
        ));
        source.seek(SeekFrom::Start(0)).unwrap();
        let mut raw_bytes = Vec::new();
        source.read_to_end(&mut raw_bytes).unwrap();
        assert_eq!(raw_bytes, encrypted);
        assert_eq!(source.into_inner().into_inner(), encrypted);
    }
}

#[test]
fn missing_wrong_cancelled_and_exhausted_credentials_never_return_a_source() {
    let bytes = fixture::encrypted_image(&original(), 5, "public test");
    for (password, expected) in [
        (None, InspectError::PasswordRequired),
        (Some("wrong"), InspectError::IncorrectPassword),
    ] {
        assert!(
            matches!(AgileSource::open(Cursor::new(&bytes),password,&mut budget(),&||false),Err(e) if e==expected)
        );
    }
    assert!(matches!(
        AgileSource::open(
            Cursor::new(&bytes),
            Some("public test"),
            &mut budget(),
            &|| true
        ),
        Err(InspectError::Cancelled)
    ));
    assert!(matches!(
        AgileSource::open(
            Cursor::new(&bytes),
            Some("public test"),
            &mut ResourceBudget::new(0, 0, 100),
            &|| false
        ),
        Err(InspectError::Limit(_))
    ));
}
