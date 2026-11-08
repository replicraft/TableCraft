//! Display-only whole seconds must preserve the exact original DATE carrier.
use super::{Engine, options};
use crate::{DateFormat, DateRepresentation, NativeFormat, ScalarPresentation, TimeFormat};
use jiff::civil::date;
use tablecraft_core::values::LegacyDateTime;

// Independent Python datetime/struct goldens: day offset from 1899-12-30,
// plus the clock fraction for positive days and minus it for negative days.
const NOW_BITS: u64 = 0x40e6_9fb1_a666_6666;

fn legacy(bits: u64) -> ScalarPresentation {
    ScalarPresentation::DateTime {
        calendar: LegacyDateTime::from_bits(bits)
            .unwrap()
            .calendar_rounded_to_nanos()
            .unwrap(),
        representation: DateRepresentation::LegacyBits(bits),
        native_format: NativeFormat::GeneralDate,
    }
}

#[test]
fn whole_second_date_goldens_display_their_intended_seconds_without_changing_bits() {
    let mut policy = options("en-US");
    policy.date = DateFormat::Iso;
    policy.time = TimeFormat::HoursMinutesSeconds;
    let engine = Engine::new(&policy).unwrap();
    for (bits, expected) in [
        (NOW_BITS, "2026-11-07 13:14:15"),
        (0x40e6_9fb1_a4fa_4fa5, "2026-11-07 13:14:00"),
        (0x40e6_9fb1_aaaa_aaab, "2026-11-07 13:15:00"),
        (0x40e6_9fb2_aaaa_aaab, "2026-11-07 14:00:00"),
        (0x40e6_9fbf_ffe7_ba37, "2026-11-07 23:59:59"),
        (0x40e6_9fc0_0000_0000, "2026-11-08"),
        (0xc0e1_d531_a666_6666, "1800-01-02 13:14:15"),
        (0xbff8_d333_3333_3333, "1899-12-29 13:14:15"),
        (0xbfe1_a666_6666_6666, "13:14:15"),
    ] {
        let original = legacy(bits);
        assert_eq!(engine.format(original), expected, "DATE {bits:016x}");
        let ScalarPresentation::DateTime { representation, .. } = original else {
            unreachable!();
        };
        assert_eq!(representation, DateRepresentation::LegacyBits(bits));
    }

    policy.time = TimeFormat::HoursMinutes;
    assert_eq!(
        Engine::new(&policy)
            .unwrap()
            .format(legacy(0x40e6_9fb1_a4fa_4fa5)),
        "2026-11-07 13:14"
    );
}

#[test]
fn raw_and_fractional_choices_preserve_the_original_date_and_decoded_fraction() {
    let mut policy = options("en-US");
    policy.date = DateFormat::Iso;
    policy.time = TimeFormat::Fractional;
    let engine = Engine::new(&policy).unwrap();
    assert_eq!(
        engine.format(legacy(NOW_BITS)),
        "2026-11-07 13:14:14.999999749"
    );

    policy.time = TimeFormat::Raw;
    assert_eq!(
        Engine::new(&policy).unwrap().format(legacy(NOW_BITS)),
        "DATE(0x40e69fb1a6666666)"
    );
    assert_eq!(
        Engine::new(&policy)
            .unwrap()
            .format(legacy(0xbfe1_a666_6666_6666)),
        "DATE(0xbfe1a66666666666)"
    );
    policy.date = DateFormat::Raw;
    policy.time = TimeFormat::HoursMinutesSeconds;
    assert_eq!(
        Engine::new(&policy).unwrap().format(legacy(NOW_BITS)),
        "DATE(0x40e69fb1a6666666)"
    );
}

#[test]
fn genuine_subsecond_values_and_extended_dates_are_not_promoted_to_whole_seconds() {
    let mut policy = options("en-US");
    policy.date = DateFormat::Iso;
    policy.time = TimeFormat::HoursMinutesSeconds;
    let engine = Engine::new(&policy).unwrap();
    // These independently encoded times are 13:14:14.25, .50 and .75.
    for bits in [
        0x40e6_9fb1_a654_3210,
        0x40e6_9fb1_a65a_4382,
        0x40e6_9fb1_a660_54f4,
    ] {
        assert_eq!(engine.format(legacy(bits)), "2026-11-07 13:14:14");
    }
    let extended = ScalarPresentation::DateTime {
        calendar: date(2026, 11, 7).at(13, 14, 14, 999_999_749),
        representation: DateRepresentation::Extended,
        native_format: NativeFormat::GeneralDate,
    };
    assert_eq!(engine.format(extended), "2026-11-07 13:14:14");
    policy.time = TimeFormat::Fractional;
    assert_eq!(
        Engine::new(&policy).unwrap().format(extended),
        "2026-11-07 13:14:14.9999997"
    );
}

#[test]
fn civil_range_endpoints_remain_displayable_for_ordinary_fractional_and_raw_choices() {
    let mut policy = options("en-US");
    policy.date = DateFormat::Iso;
    policy.time = TimeFormat::HoursMinutesSeconds;
    let engine = Engine::new(&policy).unwrap();
    for (bits, expected) in [
        (0xc124_1034_0000_0000, "0100-01-01"),
        (0x4146_9240_ffff_9ee9, "9999-12-31 23:59:59"),
    ] {
        let original = legacy(bits);
        assert_eq!(engine.format(original), expected);
        for choice in [TimeFormat::Fractional, TimeFormat::Raw] {
            let mut selected = policy.clone();
            selected.time = choice;
            assert!(!Engine::new(&selected).unwrap().format(original).is_empty());
        }
    }
}
