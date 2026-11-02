//! Independent literal storage streams and exact comparison-work controls.
use super::require_payload;
use crate::{EditError, InspectError};
use std::cell::Cell;
use tablecraft_core::limits::{Resource, ResourceBudget};

fn budget() -> ResourceBudget {
    ResourceBudget::new(0, 0, 10_000)
}

#[test]
fn exact_native_unit_streams_keep_marker_and_decoded_value_distinct() {
    for (payload, text) in [
        (&b"S\0t\0r\0a\0s\0s\0e\0"[..], "Strasse"),
        (&b"\xff\xfeStrasse"[..], "Strasse"),
        (&b"\xff\xfe\xe9"[..], "é"),
        (&b"\xff\xfe\0a\0"[..], "a"),
        (&b"\xff\xfe\0\xff\xfea\0"[..], "\u{feff}a"),
        (&b"\xff\xfe"[..], ""),
        (&b"\0\0"[..], "\0"),
    ] {
        require_payload(payload, text, &mut budget(), &|| false).unwrap();
    }
    for (payload, wrong) in [
        (&b"\xff\xfea\0"[..], "\u{feff}a"),
        (&b"\xff\xfe\0\xff\xfea\0"[..], "a"),
        (&b"\xff\xfe\xe9"[..], "e\u{301}"),
        (&b"\xff\xfeStrasse"[..], "STRASSE"),
        (&b"\xff\xfe\0a"[..], "a"),
        (&b"a"[..], "a"),
        (&b"\0\xd8"[..], "\u{10000}"),
    ] {
        assert_eq!(
            require_payload(payload, wrong, &mut budget(), &|| false),
            Err(EditError::Preservation)
        );
    }
}

#[test]
fn native_carrier_pass_is_allocation_free_and_pays_before_comparison_failure() {
    for (payload, text) in [
        (&b"S\0t\0r\0a\0s\0s\0e\0"[..], "Strasse"),
        (&b"\xff\xfeStrasse"[..], "Strasse"),
        (&b"\xff\xfe\xe9"[..], "é"),
        (&b"\xff\xfe\0a\0"[..], "a"),
        (&b"\xff\xfe\0\xff\xfea\0"[..], "\u{feff}a"),
    ] {
        let mut measured = budget();
        let polls = Cell::new(0_usize);
        require_payload(payload, text, &mut measured, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
        let cost = 10_000 - measured.remaining(Resource::Steps);
        let mut exact = ResourceBudget::new(0, 0, cost);
        require_payload(payload, text, &mut exact, &|| false).unwrap();
        assert_eq!(exact.remaining(Resource::Steps), 0);
        let mut one_less = ResourceBudget::new(0, 0, cost - 1);
        assert!(
            matches!(require_payload(payload, text, &mut one_less, &|| false), Err(EditError::Inspect(InspectError::Limit(error))) if error.resource == Resource::Steps)
        );
        for stop in [1, polls.get().div_ceil(2), polls.get()] {
            let seen = Cell::new(0_usize);
            assert_eq!(
                require_payload(payload, text, &mut budget(), &|| {
                    seen.set(seen.get() + 1);
                    seen.get() >= stop
                }),
                Err(EditError::Inspect(InspectError::Cancelled))
            );
            assert_eq!(seen.get(), stop);
        }
    }
}
