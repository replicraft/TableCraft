//! SOURCE-only nullable mask/payload expectations; no writer calls.
#[path = "nullable_sources/model.rs"]
pub(crate) mod model;

use super::{SCOPE, budget};
use std::io::Cursor;
use tablecraft_ace::{PropertyGroupKind, inspect_table_properties, read_table_projection};
use tablecraft_core::values::Value;

fn projection(bytes: &[u8]) -> Vec<Option<i32>> {
    read_table_projection(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        &["Large_Number"],
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .rows()
    .iter()
    .map(|row| match row.as_slice() {
        [Value::Null] => None,
        [Value::Int32(value)] => Some(*value),
        _ => panic!("Unexpected target carrier"),
    })
    .collect()
}

#[test]
fn qualify_preserved_nullable_int32_mask_payload_sources_before_producer() {
    let canonical = super::model::source(6, 101).before;
    // The unchanged actual owner independently qualifies all roles, metadata,
    // retained calculations, attachments and advanced payload projections.
    super::qualify(&canonical, 1000);
    let mut admitted = 0;
    let mut refused = 0;
    for name in model::CASES {
        let case = model::source(name);
        super::roles(&case.before);
        assert_eq!(projection(&case.before), case.initial);
        let properties = inspect_table_properties(
            &mut Cursor::new(&case.before),
            SCOPE,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let property = properties
            .group(PropertyGroupKind::Column, "Large_Number")
            .unwrap()
            .get("Required")
            .unwrap();
        assert_eq!(property.boolean().unwrap(), case.required);
        if case.required {
            assert!(case.initial.iter().all(Option::is_some));
            assert_eq!(case.replacement[0], None);
            assert!(case.expected.is_none());
            refused += 1;
            continue;
        }
        let expected = case.expected.as_ref().unwrap();
        super::roles(expected);
        assert_eq!(projection(expected), case.replacement);
        assert_eq!(expected.len(), case.before.len());
        let mut permitted = Vec::new();
        for ordinal in 0..3 {
            let (payload, mask, end) = model::row(&case.before, ordinal);
            assert_eq!(end, mask + 2);
            assert_eq!(
                u16::from_le_bytes(case.before[mask - 2..mask].try_into().unwrap()),
                5
            );
            let footer = mask - 2 - 12;
            let offsets = (0..6)
                .map(|index| {
                    let at = mask - 4 - index * 2;
                    usize::from(u16::from_le_bytes(
                        case.before[at..at + 2].try_into().unwrap(),
                    ))
                })
                .collect::<Vec<_>>();
            assert!(offsets.windows(2).all(|pair| pair[0] <= pair[1]));
            // All fixed bytes end at relative130 in the unchanged rich owner.
            let record_start = payload - 10;
            assert_eq!(offsets[0], 130);
            assert!(record_start + offsets[5] <= footer);
            assert!(payload + 8 <= record_start + offsets[0]);
            assert_eq!(
                &case.before[payload + 4..payload + 8],
                &expected[payload + 4..payload + 8]
            );
            assert_eq!(case.before[mask] & !16, expected[mask] & !16);
            assert_eq!(case.before[mask + 1], expected[mask + 1]);
            permitted.push(mask);
            if case.replacement[ordinal].is_some() {
                permitted.extend(payload..payload + 4);
            } else {
                assert_eq!(
                    &case.before[payload..payload + 4],
                    &expected[payload..payload + 4]
                );
            }
        }
        for (offset, (before, after)) in case.before.iter().zip(expected).enumerate() {
            assert!(
                before == after || permitted.contains(&offset),
                "Unexpected changed byte {offset}"
            );
        }
        if case.initial == case.replacement {
            assert_eq!(case.before, *expected);
        }
        assert!(case.requested_rows == 1 || case.requested_rows == 3);
        admitted += 1;
    }
    assert_eq!((admitted, refused), (6, 1));
}

#[path = "nullable_sources/api.rs"]
mod api;
