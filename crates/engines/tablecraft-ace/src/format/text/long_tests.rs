use crate::InspectError;
use std::cell::Cell;
use tablecraft_core::limits::{Resource, ResourceBudget};

fn decode_long_with_limits(
    encoded: &[u8],
    encoded_limit: usize,
    decoded_limit: usize,
    budget: &mut ResourceBudget,
) -> Result<String, InspectError> {
    super::decode_long_with_limits(encoded, encoded_limit, decoded_limit, budget, &|| false)
}

fn pool() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 0, 100_000_000)
}

#[test]
fn decoded_limits_measure_utf8_independently_of_encoded_storage() {
    for (encoded, wanted) in [
        (vec![0xff, 0xfe, b'a', b'b'], "ab"),
        (vec![0xff, 0xfe, 0xe9], "é"),
        (vec![0x2d, 0x4e], "中"),
        (vec![0x3e, 0xd8, 0x80, 0xdd], "🦀"),
        (vec![0xff, 0xfe, b'A', 0, 0x2d, 0x4e, 0, b'B'], "A中B"),
    ] {
        assert_eq!(
            decode_long_with_limits(&encoded, encoded.len(), wanted.len(), &mut pool()).unwrap(),
            wanted
        );
        assert_eq!(
            decode_long_with_limits(&encoded, encoded.len(), wanted.len() - 1, &mut pool()),
            Err(InspectError::StructureLimit)
        );
        assert_eq!(
            decode_long_with_limits(&encoded, encoded.len() - 1, wanted.len(), &mut pool()),
            Err(InspectError::StructureLimit)
        );
    }
}

#[test]
fn exact_decoded_allocation_and_refusals_preserve_cumulative_pools() {
    let mut bytes = ResourceBudget::new(9, 0, 0);
    assert_eq!(
        decode_long_with_limits(&[0x2d, 0x4e], 2, 3, &mut bytes).unwrap(),
        "中"
    );
    // Two encoded bytes reserve two UTF-16 slots (four bytes), then exactly
    // three UTF-8 bytes. The decoded ceiling is checked before String allocation.
    assert_eq!(bytes.remaining(Resource::Bytes), 2);

    let mut bytes = ResourceBudget::new(9, 0, 0);
    assert_eq!(
        decode_long_with_limits(&[0x2d, 0x4e], 2, 2, &mut bytes),
        Err(InspectError::StructureLimit)
    );
    assert_eq!(bytes.remaining(Resource::Bytes), 5);

    let mut bytes = ResourceBudget::new(0, 0, 0);
    assert_eq!(
        decode_long_with_limits(b"\xff\xfeabc", 5, 2, &mut bytes),
        Err(InspectError::StructureLimit)
    );
    assert_eq!(bytes.remaining(Resource::Bytes), 0);
    assert!(decode_long_with_limits(&[0x2d, 0x4e], 2, 3, &mut bytes).is_err());

    for malformed in [&[0][..], &[0, 0xd8], &[0xff, 0xfe, 0, 1]] {
        assert!(decode_long_with_limits(malformed, 32, 32, &mut pool()).is_err());
    }
}

#[test]
fn larger_memos_reach_the_explicit_sixteen_mib_decoded_boundary() {
    let mut encoded = vec![0xff, 0xfe];
    encoded.extend(std::iter::repeat_n(b'x', 16_777_216));
    let decoded = decode_long_with_limits(&encoded, 33_554_432, 16_777_216, &mut pool()).unwrap();
    assert_eq!(decoded.len(), 16_777_216);
    assert!(decoded.bytes().all(|byte| byte == b'x'));
    assert_eq!(
        decode_long_with_limits(&encoded, 33_554_432, 1_048_576, &mut pool()),
        Err(InspectError::StructureLimit)
    );
    encoded.push(b'x');
    assert_eq!(
        decode_long_with_limits(&encoded, 33_554_432, 16_777_216, &mut pool()),
        Err(InspectError::StructureLimit)
    );
}

#[test]
fn cancellation_interrupts_each_decode_route_after_successful_prefixes() {
    let count = 12_288;
    let mut compressed_ascii = vec![0xff, 0xfe];
    compressed_ascii.extend(std::iter::repeat_n(b'a', count));
    let ascii = "a".repeat(count);
    let bmp = "中".repeat(count);
    let supplementary = "🦀".repeat(count);
    let mut switching = vec![0xff, 0xfe];
    for _ in 0..count {
        // A, switch to UTF-16, U+4E2D, switch back to compressed.
        switching.extend_from_slice(&[b'A', 0, 0x2d, 0x4e, 0]);
    }
    let utf16 = |value: &str| {
        value
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    };

    for (route, encoded) in [
        ("compressed ASCII", compressed_ascii),
        ("UTF-16 ASCII", utf16(&ascii)),
        ("BMP", utf16(&bmp)),
        ("supplementary", utf16(&supplementary)),
        ("compressed switching", switching),
    ] {
        let calls = Cell::new(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            false
        };
        let expected = super::decode_long_with_limits(
            &encoded,
            encoded.len(),
            100_000,
            &mut pool(),
            &cancelled,
        )
        .unwrap();
        assert!(!expected.is_empty());
        let checkpoints = calls.get();
        assert!(checkpoints > 3, "{route}");

        for stop in 3..=checkpoints {
            calls.set(0);
            let cancelled = || {
                calls.set(calls.get() + 1);
                calls.get() == stop
            };
            assert_eq!(
                super::decode_long_with_limits(
                    &encoded,
                    encoded.len(),
                    100_000,
                    &mut pool(),
                    &cancelled,
                ),
                Err(InspectError::Cancelled),
                "{route} checkpoint {stop}"
            );
            assert_eq!(calls.get(), stop);
        }

        let mut low = ResourceBudget::new(0, 0, 100_000_000);
        assert!(matches!(
            super::decode_long_with_limits(
                &encoded, encoded.len(), 100_000, &mut low, &|| false,
            ),
            Err(InspectError::Limit(error)) if error.resource == Resource::Bytes
        ));
        assert_eq!(low.remaining(Resource::Bytes), 0);
    }
}
