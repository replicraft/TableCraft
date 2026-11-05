use super::*;
use crate::{
    controls::{SCOPE, budget},
    source::Source,
};
use bytes::Bytes;
use tablecraft_ace::{InspectError, inspect_table_properties};
use tablecraft_testkit::fixtures::access::{field_presentation, wide_properties};

fn properties(flags: u8, native_type: u8, scope: u16, raw: &[u8]) -> TableProperties {
    let bytes = field_presentation::database(
        &[
            wide_properties::names(&["Required"]),
            field_presentation::score_group(
                scope,
                &[field_presentation::property(0, flags, native_type, raw)],
            ),
        ],
        false,
    );
    let mut work = budget();
    let (mut source, _) = Source::open(Bytes::from(bytes), "", &mut work, &|| false).unwrap();
    inspect_table_properties(&mut source, SCOPE, "Items", &mut work, &|| false).unwrap()
}

#[test]
fn uninspected_absent_and_unsupported_properties_are_distinct() {
    let mut work = budget();
    let uninspected = inspect("Score", None, &mut work, &|| false).unwrap();
    assert!(
        uninspected
            .iter()
            .all(|property| property.value == FieldPropertyValue::Uninspected)
    );

    let index = properties(1, 1, 2, &[1]);
    let absent = inspect("Score", Some(&index), &mut work, &|| false).unwrap();
    assert!(
        absent
            .iter()
            .all(|property| property.value == FieldPropertyValue::Absent)
    );

    for (flags, native_type, raw) in [(0, 1, &[1][..]), (1, 4, &[1, 0, 0, 0][..])] {
        let properties = properties(flags, native_type, 1, raw);
        let observations = inspect("Score", Some(&properties), &mut work, &|| false).unwrap();
        assert_eq!(observations[0].name, "Required");
        assert_eq!(observations[0].value, FieldPropertyValue::Unsupported);
        assert!(
            observations[1..]
                .iter()
                .all(|property| property.value == FieldPropertyValue::Absent)
        );
    }
}

#[test]
fn malformed_boolean_width_is_not_an_unsupported_or_false_observation() {
    for raw in [&[][..], &[1, 0][..], &[1, 0, 0, 0][..]] {
        let properties = properties(1, 1, 1, raw);
        assert!(matches!(
            inspect("Score", Some(&properties), &mut budget(), &|| false),
            Err(Error::Native(InspectError::Malformed))
        ));
    }
}

#[test]
fn property_publication_checks_cancellation_and_its_actual_envelope_allocation() {
    assert!(matches!(
        inspect("Score", None, &mut budget(), &|| true),
        Err(Error::Cancelled)
    ));
    let mut work = ResourceBudget::new((7 * size_of::<FieldProperty>() - 1) as u64, 0, 1_000);
    assert!(matches!(
        inspect("Score", None, &mut work, &|| false),
        Err(Error::Limit(_))
    ));
}
