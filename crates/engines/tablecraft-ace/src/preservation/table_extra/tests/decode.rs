use super::budget;
use crate::{
    InspectError,
    preservation::table_extra::{TableExtraCoverage, decode::decode},
};
use std::cell::Cell;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_testkit::fixtures::access::wide_properties as fixture;

#[test]
fn numeric_groups_names_and_uncompressed_text_remain_independent_facts() {
    let bytes = fixture::golden();
    let original = bytes.clone();
    let result = decode(&bytes, &mut budget(), &|| false).unwrap();
    assert_eq!(result.coverage, TableExtraCoverage::WideMr2);
    assert_eq!(result.names, ["ValidateUpdate", "AfterDelete"]);
    assert_eq!(result.groups.len(), 2);
    for (group, kind, name, text) in [
        (&result.groups[0], 3, "ValidateUpdate", "BeforeChange"),
        (&result.groups[1], 4, "AfterDelete", "inert"),
    ] {
        assert_eq!(group.kind(), kind);
        let properties = group.properties().unwrap();
        assert_eq!(properties.len(), 1);
        assert_eq!(properties[0].name(), name);
        assert_eq!(properties[0].flags(), 1);
        assert_eq!(properties[0].native_type(), 12);
        assert_eq!(properties[0].raw(), fixture::text(text));
        assert_eq!(properties[0].text(), Some(text));
    }
    assert_eq!(bytes, original);

    let names_only = decode(
        &fixture::bag(&[fixture::names(&["AfterUpdate"])]),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(names_only.coverage, TableExtraCoverage::WideMr2);
    assert_eq!(names_only.names, ["AfterUpdate"]);
    assert!(names_only.groups.is_empty());
}

#[test]
fn opaque_signature_group_flags_and_types_preserve_exact_bytes() {
    let signature = decode(b"future", &mut budget(), &|| false).unwrap();
    assert_eq!(signature.coverage, TableExtraCoverage::Opaque);
    let empty = decode(&[], &mut budget(), &|| false).unwrap();
    assert_eq!(empty.coverage, TableExtraCoverage::Empty);

    for (flags, native_type) in [(0xa5, 12), (1, 0xfe), (0xa5, 0xfe)] {
        // Invalid UTF16 is deliberately opaque when flags/type are unknown.
        let bytes = fixture::bag(&[
            fixture::names(&["Opaque"]),
            fixture::group(3, &[fixture::property(0, flags, native_type, &[0xff])]),
        ]);
        let result = decode(&bytes, &mut budget(), &|| false).unwrap();
        assert_eq!(result.coverage, TableExtraCoverage::Opaque);
        let property = &result.groups[0].properties().unwrap()[0];
        assert_eq!(property.flags(), flags);
        assert_eq!(property.native_type(), native_type);
        assert_eq!(property.raw(), &[0xff]);
        assert_eq!(property.text(), None);
    }

    let result = decode(
        &fixture::bag(&[
            fixture::names(&["Opaque"]),
            fixture::block(0x1234, &[9, 8, 7]),
        ]),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(result.coverage, TableExtraCoverage::Opaque);
    assert_eq!(result.groups[0].kind(), 0x1234);
    assert_eq!(result.groups[0].raw(), &[9, 8, 7]);
    assert!(result.groups[0].properties().is_none());

    // Complete but unsupported group-name metadata must remain visible.
    let mut body = vec![12, 0, 0, 0, 2, 0, b'X', 0, 0, 0, 0, 0];
    body.extend(fixture::property(0, 1, 12, &fixture::text("inert")));
    let result = decode(
        &fixture::bag(&[fixture::names(&["A"]), fixture::block(3, &body)]),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(result.coverage, TableExtraCoverage::Opaque);
    assert_eq!(result.groups[0].raw(), body);
    assert!(result.groups[0].properties().is_none());
}

#[test]
fn each_wide_length_layer_and_strict_utf16_are_checked() {
    let name_list = fixture::names(&["A"]);
    let original = fixture::bag(&[
        name_list.clone(),
        fixture::group(3, &[fixture::property(0, 1, 12, &fixture::text("ok"))]),
    ]);
    let group = 4 + name_list.len();
    let property = group + 16;
    let mut malformed = Vec::new();
    for (offset, value) in [(4, 5_u32), (group, 5), (group + 6, 3), (property, 11)] {
        let mut bytes = original.clone();
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        malformed.push(bytes);
    }
    let mut bad_index = original.clone();
    bad_index[property + 6..property + 8].copy_from_slice(&1_u16.to_le_bytes());
    malformed.push(bad_index);
    let mut bad_data_length = original.clone();
    bad_data_length[property + 8..property + 12].copy_from_slice(&6_u32.to_le_bytes());
    malformed.push(bad_data_length);
    for payload in [vec![0x41], vec![0x00, 0xd8], vec![0x00, 0xdc]] {
        malformed.push(fixture::bag(&[
            name_list.clone(),
            fixture::group(3, &[fixture::property(0, 1, 12, &payload)]),
        ]));
    }
    for bytes in malformed {
        assert!(matches!(
            decode(&bytes, &mut budget(), &|| false),
            Err(InspectError::Malformed | InspectError::Truncated)
        ));
    }
    for end in [
        5,
        9,
        group + 3,
        group + 12,
        property + 9,
        original.len() - 1,
    ] {
        assert!(matches!(
            decode(&original[..end], &mut budget(), &|| false),
            Err(InspectError::Malformed | InspectError::Truncated)
        ));
    }
}

#[test]
fn duplicate_and_out_of_order_metadata_never_become_complete_groups() {
    let entry = fixture::property(0, 1, 12, &fixture::text("x"));
    let names = fixture::names(&["A"]);
    for blocks in [
        vec![names.clone(), names.clone()],
        vec![
            fixture::group(3, std::slice::from_ref(&entry)),
            names.clone(),
        ],
        vec![fixture::names(&["A", "A"])],
        vec![
            names.clone(),
            fixture::group(3, &[entry.clone(), entry.clone()]),
        ],
        vec![
            names,
            fixture::group(3, std::slice::from_ref(&entry)),
            fixture::group(3, &[entry]),
        ],
    ] {
        assert_eq!(
            decode(&fixture::bag(&blocks), &mut budget(), &|| false).err(),
            Some(InspectError::Malformed)
        );
    }
}

#[test]
fn decoding_pays_each_pool_and_observes_late_cancellation() {
    let bytes = fixture::golden();
    let mut measured = budget();
    decode(&bytes, &mut measured, &|| false).unwrap();
    for resource in [Resource::Bytes, Resource::Steps] {
        let initial = budget().remaining(resource);
        let consumed = initial - measured.remaining(resource);
        assert!(consumed > 0, "{resource:?}");
        let mut limits = [32 * 1024 * 1024, 16_384, 1_000_000];
        let position = match resource {
            Resource::Bytes => 0,
            Resource::Rows => 1,
            Resource::Steps => 2,
        };
        limits[position] = consumed - 1;
        let mut limited = ResourceBudget::new(limits[0], limits[1], limits[2]);
        assert!(matches!(
            decode(&bytes, &mut limited, &|| false),
            Err(InspectError::Limit(error)) if error.resource == resource
        ));
    }
    let calls = Cell::new(0);
    decode(&bytes, &mut budget(), &|| {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    assert!(calls.get() > 2);
    let stop = calls.get();
    calls.set(0);
    assert_eq!(
        decode(&bytes, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            calls.get() >= stop
        })
        .err(),
        Some(InspectError::Cancelled)
    );
}

#[test]
fn wide_metadata_preserves_large_raw_and_decoded_text_under_paid_input_bounds() {
    let payload = [0x2d, 0x4e].repeat(1_048_577);
    let bytes = fixture::bag(&[
        fixture::names(&["AfterUpdate"]),
        fixture::group(3, &[fixture::property(0, 1, 12, &payload)]),
    ]);
    let mut pool = ResourceBudget::new(64 * 1024 * 1024, 0, 20_000_000);
    let decoded = decode(&bytes, &mut pool, &|| false).unwrap();
    let property = &decoded.groups[0].properties().unwrap()[0];
    assert_eq!(property.raw(), payload);
    let text = property.text().unwrap();
    assert_eq!(text.len(), 3_145_731);
    assert!(text.chars().all(|character| character == '中'));
    assert!(matches!(
        decode(
            &bytes,
            &mut ResourceBudget::new(1_000_000, 0, 20_000_000),
            &|| false
        ),
        Err(InspectError::Limit(_))
    ));
}

#[test]
fn wide_names_groups_and_entries_follow_wire_lengths_and_paid_comparisons() {
    let labels = (0..2050)
        .map(|index| format!("P{index}"))
        .collect::<Vec<_>>();
    let names = labels.iter().map(String::as_str).collect::<Vec<_>>();
    let values = (0..2050)
        .map(|index| fixture::property(index, 0xa5, 0x7f, &[7]))
        .collect::<Vec<_>>();
    let mut blocks = vec![
        fixture::names(&names),
        fixture::group(3, &values),
        fixture::group(4, &values),
    ];
    for _ in 0..1025 {
        blocks.push(fixture::block(0x1234, &[]));
    }
    let bytes = fixture::bag(&blocks);
    let mut pool = ResourceBudget::new(32 * 1024 * 1024, 0, 200_000_000);
    let decoded = decode(&bytes, &mut pool, &|| false).unwrap();
    assert_eq!(decoded.names.len(), 2050);
    assert_eq!(decoded.groups.len(), 1027);
    assert_eq!(decoded.groups[0].properties().unwrap().len(), 2050);
    assert_eq!(decoded.groups[1].properties().unwrap().len(), 2050);
    assert!(matches!(
        decode(
            &bytes,
            &mut ResourceBudget::new(32 * 1024 * 1024, 0, 100),
            &|| false
        ),
        Err(InspectError::Limit(_))
    ));
}
