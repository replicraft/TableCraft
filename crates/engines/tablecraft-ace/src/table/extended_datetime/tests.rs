use super::*;
use proptest::prelude::*;

fn encoded(days: u64, seconds: u64, fraction: u32) -> Vec<u8> {
    format!("{days:019}:{seconds:012}{fraction:07}:7\0").into_bytes()
}

#[test]
fn independent_epoch_and_upper_bound_goldens_preserve_seven_digits() {
    let epoch = b"0000000000000000000:0000000000000000000:7\0";
    assert_eq!(epoch.len(), 42);
    let value = decode(epoch).unwrap();
    assert_eq!(value.date(), (1, 1, 1));
    assert_eq!(value.time(), (0, 0, 0, 0));
    let value = decode(&encoded(3_652_058, 86_399, 9_999_999)).unwrap();
    assert_eq!(value.date(), (9999, 12, 31));
    assert_eq!(value.time(), (23, 59, 59, 9_999_999));
    let value = decode(&encoded(730_178, 45_296, 1_234_567)).unwrap();
    assert_eq!(value.date(), (2000, 2, 29));
    assert_eq!(value.time(), (12, 34, 56, 1_234_567));
}

#[test]
fn noncanonical_digits_delimiters_ranges_and_lengths_are_rejected() {
    let original = encoded(0, 0, 0);
    for position in 0..42 {
        let mut bytes = original.clone();
        bytes[position] = b'A';
        assert_eq!(decode(&bytes), Err(InspectError::Malformed));
    }
    for bytes in [
        encoded(3_652_059, 0, 0),
        encoded(0, 86_400, 0),
        vec![b'0'; 41],
        vec![b'0'; 43],
    ] {
        assert_eq!(decode(&bytes), Err(InspectError::Malformed));
    }
}

#[cfg(feature = "ace16-write-preview")]
#[test]
fn constructor_validated_carriers_encode_exact_independent_native_goldens() {
    for (date, time, days, seconds) in [
        ((1, 1, 1), (0, 0, 0, 0), 0, 0),
        ((1, 1, 1), (0, 0, 0, 1), 0, 0),
        ((1900, 3, 1), (0, 0, 0, 0), 693_654, 0),
        ((2000, 2, 29), (12, 34, 56, 1_234_567), 730_178, 45_296),
        ((9999, 12, 31), (23, 59, 59, 9_999_999), 3_652_058, 86_399),
    ] {
        let value =
            ExtendedDateTime::new(date.0, date.1, date.2, time.0, time.1, time.2, time.3).unwrap();
        let actual = encode(&value).unwrap();
        assert_eq!(actual.as_slice(), encoded(days, seconds, time.3));
        assert_eq!(decode(&actual).unwrap(), value);
    }
}

fn days_before(year: u16, month: u8, day: u8) -> u64 {
    // Independent elementary Gregorian count, not the production Jiff path.
    let completed = u64::from(year - 1);
    let mut days = completed * 365 + completed / 4 - completed / 100 + completed / 400;
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let lengths = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    for length in &lengths[..usize::from(month - 1)] {
        days += length;
    }
    days + u64::from(day - 1)
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x4143454558544441),..Default::default()})]
    #[test]
    fn canonical_dates_agree_with_independent_gregorian_arithmetic(year in 1_u16..=9999, month in 1_u8..=12, day in 1_u8..=28, hour in 0_u8..24, minute in 0_u8..60, second in 0_u8..60, fraction in 0_u32..10_000_000) {
        let seconds = u64::from(hour) * 3600 + u64::from(minute) * 60 + u64::from(second);
        let decoded = decode(&encoded(days_before(year, month, day), seconds, fraction)).unwrap();
        prop_assert_eq!(decoded.date(), (year, month, day));
        prop_assert_eq!(decoded.time(), (hour, minute, second, fraction));
    }
    #[test]
    fn arbitrary_native_timestamp_bytes_never_panic(bytes in prop::collection::vec(any::<u8>(), 0..100)) {
        let _ = decode(&bytes);
    }
}
