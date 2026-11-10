//! Retained dictionaries are explicit current-schema evidence, not legacy admission.
use super::*;
use crate::inspect_table_properties;
use std::io::Cursor;
use tablecraft_core::limits::Resource;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::deleted_field::populated as raw;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 10_000_000)
}

fn properties(bytes: &[u8]) -> TableProperties {
    inspect_table_properties(
        &mut Cursor::new(bytes),
        CatalogScope::new(1751),
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn retained_dictionary_and_aligned_groups_are_complete_without_widening_legacy_framing() {
    for case in raw::CASES {
        let [before, after] = raw::pair(case);
        let original = properties(&before);
        let current = properties(&after);
        validate(&original, &mut budget(), &|| false).unwrap();
        let names = current_schema_names(&current, &mut budget(), &|| false).unwrap();
        assert_eq!(
            names,
            ordered_names(&original, &mut budget(), &|| false).unwrap()
        );
        let mut spans = Vec::new();
        let visited_names =
            current_schema_group_blocks(&current, &mut budget(), &|| false, |group, span| {
                assert_eq!(group, spans.len());
                let declared = u32::from_le_bytes(
                    current.raw_bytes()[span.start..span.start + 4]
                        .try_into()
                        .unwrap(),
                ) as usize;
                assert_eq!(span.len(), declared);
                spans.push(span);
                Ok(())
            })
            .unwrap();
        assert_eq!(visited_names, names);
        assert_eq!(spans.len(), current.groups().len());
        if matches!(
            case,
            raw::Case::TextPresent | raw::Case::TextAbsent | raw::Case::IdOnlyText
        ) {
            assert!(matches!(
                validate(&current, &mut budget(), &|| false),
                Err(InspectError::Unsupported)
            ));
        } else {
            validate(&current, &mut budget(), &|| false).unwrap();
        }
    }
}

#[test]
fn current_framing_retains_tail_reference_budget_and_cancellation_checks() {
    let [_, after] = raw::pair(raw::Case::TextAbsent);
    for resource in [Resource::Bytes, Resource::Steps] {
        let mut pool = match resource {
            Resource::Bytes => ResourceBudget::new(0, 100, 100_000),
            Resource::Steps => ResourceBudget::new(100_000, 100, 0),
            Resource::Rows => unreachable!(),
        };
        assert!(
            matches!(current_schema_names(&properties(&after), &mut pool, &|| false), Err(InspectError::Limit(error)) if error.resource == resource)
        );
    }
    assert!(matches!(
        current_schema_names(&properties(&after), &mut budget(), &|| true),
        Err(InspectError::Cancelled)
    ));

    // An internal decoded view is data only. Frame validation must still reject
    // raw bytes that the view does not describe; no Source can be constructed here.
    let mut trailing = properties(&after);
    trailing.raw.push(0x5a);
    assert!(current_schema_names(&trailing, &mut budget(), &|| false).is_err());

    let mut invalid_reference = properties(&after);
    let mut first = None;
    current_schema_group_blocks(&invalid_reference, &mut budget(), &|| false, |_, span| {
        first.get_or_insert(span.start);
        Ok(())
    })
    .unwrap();
    let group = &invalid_reference.groups()[0];
    // Independent MR2 group/value widths, not production parser constants.
    let name_index = first.unwrap() + 6 + 4 + 2 + group.name().encode_utf16().count() * 2 + 4;
    invalid_reference.raw[name_index..name_index + 2].copy_from_slice(&u16::MAX.to_le_bytes());
    assert!(matches!(
        current_schema_names(&invalid_reference, &mut budget(), &|| false),
        Err(InspectError::Malformed)
    ));
}
