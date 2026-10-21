use super::*;
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
    use std::sync::atomic::{AtomicUsize, Ordering};
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
        assert_eq!(ClockFunction::Now.apply(Some(&clock)), Err(error));
        assert_eq!(clock.calls.load(Ordering::SeqCst), 1);
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
