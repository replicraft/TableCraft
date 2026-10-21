use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn reading() -> ClockReading {
    ClockReading::new(ExtendedDateTime::new(1969, 2, 12, 16, 35, 17, 9_999_999).unwrap()).unwrap()
}

#[test]
fn clock_readings_project_whole_seconds_and_reject_legacy_range_gaps() {
    let reading = reading();
    assert_eq!(reading.date_time().time(), (16, 35, 17, 0));
    assert_eq!(
        ClockReading::new(ExtendedDateTime::new(1, 1, 1, 0, 0, 0, 0).unwrap()),
        Err(EvalError::Overflow)
    );
    for (function, expected) in [
        (ClockFunction::Now, "1969-02-12T16:35:17"),
        (ClockFunction::Date, "1969-02-12T00:00:00"),
        (ClockFunction::Time, "1899-12-30T16:35:17"),
    ] {
        let expected = LegacyDateTime::from_calendar(expected.parse().unwrap()).unwrap();
        assert_eq!(
            function.apply(Some(&reading)),
            Ok(Value::DateTime(expected))
        );
        assert_eq!(function.apply(None), Err(EvalError::ClockRequired));
    }
}

#[test]
fn callback_failures_propagate_without_fallback_or_retry() {
    struct Failing {
        calls: AtomicUsize,
        error: EvalError,
    }
    impl Clock for Failing {
        fn read(&self) -> Result<ClockReading, EvalError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(self.error)
        }
    }
    for error in [
        EvalError::ClockUnavailable,
        EvalError::Cancelled,
        EvalError::Domain,
    ] {
        let clock = Failing {
            calls: AtomicUsize::new(0),
            error,
        };
        for function in [ClockFunction::Now, ClockFunction::Timer] {
            assert_eq!(function.apply(Some(&clock)), Err(error));
        }
        assert_eq!(clock.calls.load(Ordering::SeqCst), 2);
    }
}

#[test]
fn epoch_sign_projection_and_calendar_endpoints_stay_valid() {
    let reading =
        ClockReading::new(ExtendedDateTime::new(1899, 12, 29, 6, 0, 0, 0).unwrap()).unwrap();
    for (function, serial) in [
        (ClockFunction::Now, -1.25),
        (ClockFunction::Date, -1.0),
        (ClockFunction::Time, 0.25),
    ] {
        assert_eq!(
            function.apply(Some(&reading)),
            Ok(Value::DateTime(
                LegacyDateTime::from_ole_days(serial).unwrap()
            ))
        );
    }
    for value in [
        ExtendedDateTime::new(100, 1, 1, 0, 0, 0, 0).unwrap(),
        ExtendedDateTime::new(9999, 12, 31, 23, 59, 59, 9_999_999).unwrap(),
    ] {
        let reading = ClockReading::new(value).unwrap();
        for function in [ClockFunction::Now, ClockFunction::Date, ClockFunction::Time] {
            assert!(function.apply(Some(&reading)).is_ok());
        }
    }
}

#[test]
fn timer_preserves_single_seconds_without_date_precision_loss() {
    for year in [100, 1899, 2026, 9999] {
        for (hour, minute, second, fraction, expected) in [
            (0, 0, 0, 0, 0.0_f32),
            (0, 0, 0, 1, 0.000_000_1_f32),
            (0, 0, 1, 2_500_000, 1.25_f32),
            (13, 14, 15, 1_250_000, 47_655.125_f32),
            (23, 59, 59, 9_999_999, 86_400.0_f32),
        ] {
            let value = ExtendedDateTime::new(year, 1, 1, hour, minute, second, fraction).unwrap();
            let clock = ClockReading::new(value).unwrap();

            assert_eq!(
                ClockFunction::Timer.apply(Some(&clock)),
                Ok(Value::Single(Single::from_float(expected)))
            );
            assert_eq!(clock.date_time().time(), (hour, minute, second, 0));
        }
    }

    assert_eq!(
        ClockFunction::Timer.apply(None),
        Err(EvalError::ClockRequired)
    );
    assert_eq!(ClockFunction::resolve("tImEr"), Some(ClockFunction::Timer));
    assert_eq!(ClockFunction::resolve("Timer$"), None);
}

#[test]
fn timer_straddles_single_rounding_boundaries_and_resets_at_midnight() {
    // At 65536, adjacent binary32 values are 1/128 second apart.
    // These exact 100-nanosecond inputs straddle two halfway boundaries.
    for (fraction, expected) in [
        (39_062, 0x4780_0000),
        (39_063, 0x4780_0001),
        (117_187, 0x4780_0001),
        (117_188, 0x4780_0002),
        (156_250, 0x4780_0002),
    ] {
        let value = ExtendedDateTime::new(2026, 1, 1, 18, 12, 16, fraction).unwrap();
        let clock = ClockReading::new(value).unwrap();
        assert_eq!(clock.timer_seconds(), Single::from_bits(expected));
    }

    let midnight =
        ClockReading::new(ExtendedDateTime::new(2026, 1, 2, 0, 0, 0, 0).unwrap()).unwrap();
    assert_eq!(midnight.timer_seconds(), Single::from_float(0.0));
}
