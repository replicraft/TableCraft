use super::*;
use proptest::prelude::*;

const PROPERTY_CASES: u32 = 512;

fn date(serial: f64) -> LegacyDateTime {
    LegacyDateTime::from_ole_days(serial).unwrap()
}

fn admitted(
    value: LegacyDateTime,
    lower: Bound<LegacyDateTime>,
    upper: Bound<LegacyDateTime>,
) -> bool {
    let above = match lower {
        Bound::Unbounded => true,
        Bound::Included(bound) => !value.temporal_cmp(bound).is_lt(),
        Bound::Excluded(bound) => value.temporal_cmp(bound).is_gt(),
    };
    let below = match upper {
        Bound::Unbounded => true,
        Bound::Included(bound) => !value.temporal_cmp(bound).is_gt(),
        Bound::Excluded(bound) => value.temporal_cmp(bound).is_lt(),
    };
    above && below
}

#[test]
fn inclusive_points_keep_calendar_aliases_and_both_zero_encodings() {
    for target in [-0.25_f64, 0.25, -0.0, 0.0, -1.25, 45_000.125] {
        let selection = DateTimeIndexSelection::CalendarCandidates {
            lower: Bound::Included(date(target)),
            upper: Bound::Included(date(target)),
        }
        .keyed();
        assert!(selection.contains(Some(key(target))));
        assert!(!selection.contains(None));
        if target.trunc() == 0.0 {
            assert!(selection.contains(Some(key(-target))));
        } else {
            assert!(!selection.contains(Some(key(-target))));
            assert!(!selection.contains(Some(key(target.next_up()))));
            assert!(!selection.contains(Some(key(target.next_down()))));
        }
    }
    let zero = DateTimeIndexSelection::CalendarCandidates {
        lower: Bound::Included(date(-0.0)),
        upper: Bound::Included(date(0.0)),
    }
    .keyed();
    assert!(!zero.contains(Some(key(f64::from_bits(1)))));
    assert!(!zero.contains(Some(key(-f64::from_bits(1)))));
}

#[test]
fn reversed_and_exclusive_alias_intervals_are_empty_in_temporal_order() {
    for (lower, upper) in [
        (Bound::Included(date(-1.75)), Bound::Included(date(-1.25))),
        (Bound::Excluded(date(-0.25)), Bound::Included(date(0.25))),
        (Bound::Included(date(-0.0)), Bound::Excluded(date(0.0))),
    ] {
        let selection = DateTimeIndexSelection::CalendarCandidates { lower, upper }.keyed();
        for candidate in [None, Some(i64::MIN), Some(-1), Some(0), Some(i64::MAX)] {
            assert!(!selection.contains(candidate));
        }
    }
}

#[test]
fn boundary_days_cover_negative_time_reversal_epoch_aliases_and_year_limits() {
    let dates = [
        -657_434.999_999_999_9_f64,
        -2.75,
        -2.0,
        -1.75,
        -1.25,
        -1.0,
        -0.75,
        -0.25,
        -0.0,
        0.0,
        f64::from_bits(1),
        0.25,
        0.75,
        1.0,
        1.25,
        45_000.125,
        2_958_465.999_999_999_5,
    ]
    .map(date);
    let mut bounds = vec![Bound::Unbounded];
    for value in dates {
        bounds.push(Bound::Included(value));
        bounds.push(Bound::Excluded(value));
    }
    for &lower in &bounds {
        for &upper in &bounds {
            let selection = DateTimeIndexSelection::CalendarCandidates { lower, upper }.keyed();
            for value in dates {
                if admitted(value, lower, upper) {
                    assert!(selection.contains(Some(key(value.ole_days()))));
                }
            }
            assert!(!selection.contains(None));
        }
    }
}

#[test]
fn complete_null_and_unbounded_non_null_profiles_remain_distinct() {
    assert!(DateTimeIndexSelection::All.keyed().contains(None));
    assert!(DateTimeIndexSelection::Null.keyed().contains(None));
    assert!(
        !DateTimeIndexSelection::Null
            .keyed()
            .contains(Some(key(0.0)))
    );
    let non_null = DateTimeIndexSelection::CalendarCandidates {
        lower: Bound::Unbounded,
        upper: Bound::Unbounded,
    }
    .keyed();
    assert!(!non_null.contains(None));
    assert!(non_null.contains(Some(key(-0.0))));
    assert!(non_null.contains(Some(key(0.0))));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPERTY_CASES))]

    #[test]
    fn every_admitted_temporal_value_is_in_the_physical_candidate_interval(
        raw in -657_434.999_f64..2_958_465.999_f64,
        left in -657_434.999_f64..2_958_465.999_f64,
        right in -657_434.999_f64..2_958_465.999_f64,
        include_left in any::<bool>(),
        include_right in any::<bool>(),
    ) {
        let value = date(raw);
        let lower = if include_left {
            Bound::Included(date(left))
        } else {
            Bound::Excluded(date(left))
        };
        let upper = if include_right {
            Bound::Included(date(right))
        } else {
            Bound::Excluded(date(right))
        };
        let selection = DateTimeIndexSelection::CalendarCandidates { lower, upper }.keyed();
        if admitted(value, lower, upper) {
            prop_assert!(selection.contains(Some(key(raw))));
        }
    }
}
