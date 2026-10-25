use super::{decode, decode_long_with_limits};
use crate::InspectError;
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_048_576, 0, 0)
}

#[test]
fn wide_segments_preserve_every_nonzero_low_byte_with_zero_high_byte() {
    for unit in 1_u16..=255 {
        let encoded = [0xff, 0xfe, 0, unit as u8, 0];
        let wanted = char::from_u32(u32::from(unit)).unwrap().to_string();
        assert_eq!(decode(&encoded, true, &mut budget()).unwrap(), wanted);
        assert_eq!(
            decode_long_with_limits(&encoded, 5, wanted.len(), &mut budget(), &|| false).unwrap(),
            wanted
        );
    }
}

#[test]
fn switches_apply_at_code_unit_boundaries_not_inside_wide_units() {
    for (encoded, wanted) in [
        (
            &[0xff, 0xfe, b'A', 0, b'B', 0, b'C', 0, 0, b'D'][..],
            "ABCD",
        ),
        (&[0xff, 0xfe, 0, 0, b'A'][..], "A"),
        (&[0xff, 0xfe, 0, 0, 1, 0][..], "\u{1}"),
        (&[0xff, 0xfe, 0, 0x3e, 0xd8, 0x80, 0xdd, b'A', 0][..], "🦀A"),
    ] {
        assert_eq!(decode(encoded, true, &mut budget()).unwrap(), wanted);
    }

    // Uncompressed NUL is data; a zero at a compressed character boundary is a
    // mode transition and must not be invented as a stored NUL unit.
    assert_eq!(decode(&[0, 0], false, &mut budget()).unwrap(), "\0");
    for encoded in [
        &[0xff, 0xfe, 0, b'A'][..],
        &[0xff, 0xfe, 0, 1, 0xd8][..],
        &[0xff, 0xfe, 0, 1, 0xdc][..],
        &[0xff, 0xfe, 0, 0x3e, 0xd8, b'A', 0][..],
    ] {
        assert!(decode(encoded, true, &mut budget()).is_err());
    }
}

#[test]
fn wide_ascii_still_obeys_encoded_decoded_allocation_and_cancellation_limits() {
    let encoded = [0xff, 0xfe, 0, b'A', 0];
    assert_eq!(
        decode_long_with_limits(&encoded, 4, 1, &mut budget(), &|| false),
        Err(InspectError::StructureLimit)
    );
    assert_eq!(
        decode_long_with_limits(&encoded, 5, 0, &mut budget(), &|| false),
        Err(InspectError::StructureLimit)
    );
    assert!(
        decode_long_with_limits(&encoded, 5, 1, &mut ResourceBudget::new(10, 0, 0), &|| {
            false
        },)
        .is_err()
    );
    assert_eq!(
        decode_long_with_limits(&encoded, 5, 1, &mut ResourceBudget::new(11, 0, 0), &|| {
            false
        })
        .unwrap(),
        "A"
    );

    let mut large = vec![0xff, 0xfe, 0];
    for _ in 0..8192 {
        large.extend([b'A', 0]);
    }
    let checks = Cell::new(0);
    let cancelled = || {
        checks.set(checks.get() + 1);
        checks.get() >= 3
    };
    assert_eq!(
        decode_long_with_limits(&large, large.len(), 8192, &mut budget(), &cancelled),
        Err(InspectError::Cancelled)
    );
}
