use flate2::{Compression, write::ZlibEncoder};
use proptest::prelude::*;
use std::{cell::Cell, io::Write};
use tablecraft_ace::{AttachmentEncoding, InspectError, decode_attachment};
use tablecraft_core::limits::ResourceBudget;

fn budget() -> ResourceBudget {
    ResourceBudget::new(2 * 1024 * 1024, 100, 2_000_000)
}
fn encoded(label: &str, data: &[u8], compressed: bool) -> Vec<u8> {
    let label = label.encode_utf16().chain([0]).collect::<Vec<_>>();
    let mut body = ((12 + 2 * label.len()) as u32).to_le_bytes().to_vec();
    body.extend(1_u32.to_le_bytes());
    body.extend((label.len() as u32).to_le_bytes());
    body.extend(label.into_iter().flat_map(u16::to_le_bytes));
    body.extend(data);
    let mut result = u32::from(compressed).to_le_bytes().to_vec();
    result.extend((body.len() as u32).to_le_bytes());
    if compressed {
        let mut stream = ZlibEncoder::new(Vec::new(), Compression::fast());
        stream.write_all(&body).unwrap();
        result.extend(stream.finish().unwrap());
    } else {
        result.extend(body);
    }
    result
}

#[test]
fn independent_zlib_golden_retains_exact_binary_and_wrapper_type() {
    // Python zlib level-3 encoding of an independently written native wrapper.
    let golden = [
        1, 0, 0, 0, 24, 0, 0, 0, 120, 94, 19, 97, 96, 96, 96, 4, 98, 22, 32, 78, 98, 200, 100, 200,
        3, 210, 12, 140, 76, 255, 1, 15, 116, 2, 85,
    ];
    let result = decode_attachment(&golden, &mut budget(), &|| false).unwrap();
    assert_eq!(result.data(), [0, 1, 2, 255]);
    assert_eq!(result.type_label(), "bin");
    assert_eq!(result.encoding(), AttachmentEncoding::Zlib);
    for end in 0..golden.len() {
        assert!(decode_attachment(&golden[..end], &mut budget(), &|| false).is_err());
    }
    for append in [vec![0], golden[8..].to_vec()] {
        let mut changed = golden.to_vec();
        changed.extend(append);
        assert_eq!(
            decode_attachment(&changed, &mut budget(), &|| false).err(),
            Some(InspectError::Malformed)
        );
    }
    for (offset, value) in [(4, 23), (4, 25), (34, 0)] {
        let mut changed = golden;
        changed[offset] = value;
        assert_eq!(
            decode_attachment(&changed, &mut budget(), &|| false).err(),
            Some(InspectError::Malformed)
        );
    }
}

#[test]
fn empty_unicode_embedded_nul_and_maximum_payloads_remain_exact() {
    for compressed in [false, true] {
        for label in ["", "é🦀", "a\0b"] {
            let result =
                decode_attachment(&encoded(label, &[], compressed), &mut budget(), &|| false)
                    .unwrap();
            assert!(result.data().is_empty());
            assert_eq!(result.type_label(), label);
        }
    }
    let data = vec![42; 65_536];
    assert_eq!(
        decode_attachment(&encoded("bin", &data, true), &mut budget(), &|| false)
            .unwrap()
            .data(),
        data
    );
    assert_eq!(
        decode_attachment(&encoded("bin", &data, false), &mut budget(), &|| false).err(),
        Some(InspectError::StructureLimit)
    );
    assert_eq!(
        decode_attachment(
            &encoded("bin", &vec![42; 65_537], true),
            &mut budget(),
            &|| false
        )
        .err(),
        Some(InspectError::StructureLimit)
    );
}

#[test]
fn metadata_variants_lengths_checksum_bombs_and_bad_unicode_are_rejected() {
    let original = encoded("bin", &[0, 1, 2, 255], false);
    for (offset, value, error) in [
        (0, 2, InspectError::Unsupported),
        (12, 2, InspectError::Unsupported),
        (8, 19, InspectError::Malformed),
        (16, 0, InspectError::Malformed),
        (26, 1, InspectError::Malformed),
    ] {
        let mut changed = original.clone();
        changed[offset] = value;
        assert_eq!(
            decode_attachment(&changed, &mut budget(), &|| false).err(),
            Some(error)
        );
    }
    let mut changed = original.clone();
    changed[20..22].copy_from_slice(&0xd800_u16.to_le_bytes());
    assert_eq!(
        decode_attachment(&changed, &mut budget(), &|| false).err(),
        Some(InspectError::Malformed)
    );
    let mut changed = encoded("bin", &vec![0; 65_536], true);
    changed[4..8].copy_from_slice(&20_u32.to_le_bytes());
    assert_eq!(
        decode_attachment(&changed, &mut budget(), &|| false).err(),
        Some(InspectError::Malformed)
    );
    changed[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        decode_attachment(&changed, &mut budget(), &|| false).err(),
        Some(InspectError::StructureLimit)
    );
}

#[test]
fn resource_limits_and_cancellation_before_and_during_inflate_publish_no_payload() {
    let bytes = encoded("bin", &vec![42; 65_536], true);
    assert_eq!(
        decode_attachment(&bytes, &mut budget(), &|| true).err(),
        Some(InspectError::Cancelled)
    );
    assert!(matches!(
        decode_attachment(&bytes, &mut ResourceBudget::new(100, 100, 100), &|| false),
        Err(InspectError::Limit(_))
    ));
    let calls = Cell::new(0);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() > 25
    };
    assert_eq!(
        decode_attachment(&bytes, &mut budget(), &cancelled).err(),
        Some(InspectError::Cancelled)
    );
    assert!(calls.get() > 25);
}

proptest! {
    #![proptest_config(ProptestConfig{cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x415454414348),..Default::default()})]
    #[test]
    fn bounded_files_survive_both_storage_modes_without_interpreting_their_content(data in prop::collection::vec(any::<u8>(),0..8192),compressed in any::<bool>()) {
        let source=encoded("bin",&data,compressed);
        let result=decode_attachment(&source,&mut budget(),&||false).unwrap();
        prop_assert_eq!(result.data(),data);prop_assert_eq!(result.type_label(),"bin");
    }
}
