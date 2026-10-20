#[path = "support/deployment.rs"]
mod fixture;
use std::io::{Cursor, Read};
use tablecraft_access::{AccessInput, AccessInputError};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::ResourceBudget;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 10000, 10000)
}
fn database() -> Vec<u8> {
    include_bytes!("../../../../docs/examples/Access_Example_VBA.accdb").to_vec()
}
#[test]
fn uncompressed_and_mszip_roundtrip_exact_bytes_and_remain_read_only() {
    let expected = database();
    for compressed in [false, true] {
        let bytes = fixture::cabinet(&expected, compressed);
        let mut source = AccessInput::open(Cursor::new(&bytes), &mut budget(), &|| false).unwrap();
        assert!(source.is_deployment());
        let mut actual = Vec::new();
        source.read_to_end(&mut actual).unwrap();
        assert_eq!(actual, expected);
    }
    let source = AccessInput::open(Cursor::new(&expected), &mut budget(), &|| false).unwrap();
    assert!(!source.is_deployment());
}
#[test]
fn rejects_truncation_ambiguous_split_unsafe_and_oversized_packages() {
    let original = fixture::cabinet(&database(), false);
    for at in [0, 3, 35, 43, 60, original.len() - 1] {
        assert!(AccessInput::open(Cursor::new(&original[..at]), &mut budget(), &|| false).is_err());
    }
    for (offset, replacement) in [
        (26, vec![2, 0]),
        (28, vec![2, 0]),
        (30, vec![1, 0]),
        (34, vec![1, 0]),
        (48, 1u32.to_le_bytes().to_vec()),
        (44, (65u32 * 1024 * 1024).to_le_bytes().to_vec()),
        (60, b"../".to_vec()),
        (40, vec![0, 0]),
    ] {
        let mut bytes = original.clone();
        bytes[offset..offset + replacement.len()].copy_from_slice(&replacement);
        assert!(
            AccessInput::open(Cursor::new(bytes), &mut budget(), &|| false).is_err(),
            "offset {offset}"
        );
    }
}
#[test]
fn budget_and_cancellation_fail_before_publishing_a_database() {
    let bytes = fixture::cabinet(&database(), true);
    assert!(matches!(
        AccessInput::open(
            Cursor::new(&bytes),
            &mut ResourceBudget::new(1, 0, 10000),
            &|| false
        ),
        Err(AccessInputError::Native(InspectError::Limit(_)))
    ));
    let calls = std::cell::Cell::new(0);
    assert!(matches!(
        AccessInput::open(Cursor::new(&bytes), &mut budget(), &|| {
            calls.set(calls.get() + 1);
            calls.get() == 30
        }),
        Err(AccessInputError::Native(InspectError::Cancelled))
    ));
}

#[test]
fn reserved_header_and_signature_overlay_do_not_become_database_bytes() {
    let expected = database();
    let mut bytes = fixture::cabinet(&expected, true);
    // Original independent CAB reservation geometry: 20 header bytes, no
    // folder/data reservations. Placeholder bytes are not a valid signature.
    bytes[30..32].copy_from_slice(&4u16.to_le_bytes());
    bytes.splice(36..36, [20, 0, 0, 0].into_iter().chain([0; 20]));
    bytes[16..20].copy_from_slice(&68u32.to_le_bytes());
    let block = u32::from_le_bytes(bytes[60..64].try_into().unwrap()) + 24;
    bytes[60..64].copy_from_slice(&block.to_le_bytes());
    let size = bytes.len() as u32;
    bytes[8..12].copy_from_slice(&size.to_le_bytes());
    bytes.extend_from_slice(b"unverified signature overlay");
    let mut source = AccessInput::open(Cursor::new(bytes), &mut budget(), &|| false).unwrap();
    let mut actual = Vec::new();
    source.read_to_end(&mut actual).unwrap();
    assert_eq!(actual, expected);
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(128))]
    #[test]
    fn corrupt_metadata_and_compressed_data_do_not_panic(at in 0usize..1024, value in proptest::prelude::any::<u8>()) {
        // Small original ACE header image; parsing must survive damaged CAB
        // metadata and random deflate bytes without publishing partial data.
        let mut database = vec![0; 3 * 4096];
        database[..4].copy_from_slice(&[0, 1, 0, 0]);
        database[4..19].copy_from_slice(b"Standard ACE DB");
        database[20] = 5;
        let mut bytes = fixture::cabinet(&database, true);
        let offset = at % bytes.len();
        bytes[offset] ^= value;
        let _ = AccessInput::open(Cursor::new(bytes), &mut budget(), &|| false);
    }
}
