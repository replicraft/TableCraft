use super::*;
use proptest::prelude::*;
use std::cell::Cell;
use tablecraft_core::limits::LimitError;

struct Request(usize);

impl RowAssignment for Request {
    fn row(&self) -> usize {
        self.0
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(2 * 1024 * 1024, 0, 1_000_000)
}

proptest! {
    #[test]
    fn monotone_lookup_matches_independent_first_request_search(
        ordinals in prop::collection::vec(0_usize..128, 0..256),
    ) {
        let requests: Vec<_> = ordinals.iter().copied().map(Request).collect();
        let mut work = budget();
        let mut directory = Directory::new(&requests, &mut work, &|| false).unwrap();
        for ordinal in 0..129 {
            let expected = ordinals.iter().position(|&value| value == ordinal);
            prop_assert_eq!(directory.at(ordinal, &mut work, &|| false).unwrap(), expected);
        }

        let duplicate = ordinals.iter().enumerate().any(|(position, value)| {
            ordinals[..position].contains(value)
        });
        prop_assert_eq!(
            directory.require_unique(&mut work, &|| false).is_err(),
            duplicate,
        );
    }
}

#[test]
fn missing_maximum_and_duplicate_requests_retain_original_positions() {
    let requests = [Request(3), Request(1), Request(1), Request(usize::MAX)];
    let mut work = budget();
    let mut directory = Directory::new(&requests, &mut work, &|| false).unwrap();
    let actual: Vec<_> = (0..5)
        .map(|ordinal| directory.at(ordinal, &mut work, &|| false).unwrap())
        .collect();
    assert_eq!(actual, [None, Some(1), None, Some(0), None]);
    assert_eq!(
        directory.at(usize::MAX, &mut work, &|| false).unwrap(),
        Some(3)
    );
    assert!(matches!(
        directory.require_unique(&mut work, &|| false),
        Err(EditError::Conflict)
    ));
}

#[test]
fn full_sixteen_thousand_request_walk_fits_a_finite_work_allowance() {
    let requests: Vec<_> = (0..16_384).rev().map(Request).collect();
    let mut work = budget();
    let mut directory = Directory::new(&requests, &mut work, &|| false).unwrap();
    directory.require_unique(&mut work, &|| false).unwrap();
    for ordinal in 0..16_384 {
        assert_eq!(
            directory.at(ordinal, &mut work, &|| false).unwrap(),
            Some(16_383 - ordinal)
        );
    }
    assert!(work.remaining(Resource::Steps) > 0);
    assert_eq!(work.remaining(Resource::Rows), 0);
}

#[test]
fn allocation_work_and_mid_setup_cancellation_refuse_before_directory_publication() {
    let requests: Vec<_> = (0..128).map(Request).collect();
    for (mut work, resource) in [
        (ResourceBudget::new(0, 0, 1_000_000), Resource::Bytes),
        (ResourceBudget::new(1024 * 1024, 0, 0), Resource::Steps),
    ] {
        let result = Directory::new(&requests, &mut work, &|| false);
        assert!(matches!(
            result,
            Err(EditError::Inspect(InspectError::Limit(LimitError {
                resource: actual,
                ..
            }))) if actual == resource
        ));
    }

    let calls = Cell::new(0);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() >= 7
    };
    assert!(matches!(
        Directory::new(&requests, &mut budget(), &cancelled),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
}

#[test]
fn cancelled_lookup_returns_no_original_request() {
    let mut work = budget();
    let mut directory = Directory::new(&[Request(0)], &mut work, &|| false).unwrap();
    assert!(matches!(
        directory.at(0, &mut work, &|| true),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert_eq!(directory.at(0, &mut work, &|| false).unwrap(), Some(0));
}

#[test]
fn generated_sorted_population_has_linear_work_and_cancellable_validation() {
    let requests: Vec<_> = (0..16_384).map(Request).collect();
    let mut work = ResourceBudget::new(1024 * 1024, 0, 60_000);
    let mut directory = Directory::new(&requests, &mut work, &|| false).unwrap();
    assert!(matches!(
        directory.require_unique(&mut work, &|| true),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    directory.require_unique(&mut work, &|| false).unwrap();
    for ordinal in 0..requests.len() {
        assert_eq!(
            directory.at(ordinal, &mut work, &|| false).unwrap(),
            Some(ordinal)
        );
    }
    assert!(work.remaining(Resource::Steps) > 0);
}
