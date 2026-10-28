use super::*;
use proptest::prelude::*;
use std::io::Cursor;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as raw;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_048_576, 1000, 100_000)
}
fn name(text: &str) -> Vec<u8> {
    let encoded = text
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    [(encoded.len() as u16).to_le_bytes().as_slice(), &encoded].concat()
}
fn block(kind: u16, body: &[u8]) -> Vec<u8> {
    [
        ((body.len() + 6) as u32).to_le_bytes().as_slice(),
        &kind.to_le_bytes(),
        body,
    ]
    .concat()
}
fn value(index: u16, kind: u8, flags: u8, data: &[u8]) -> Vec<u8> {
    [
        ((data.len() + 8) as u16).to_le_bytes().as_slice(),
        &[flags, kind],
        &index.to_le_bytes(),
        &(data.len() as u16).to_le_bytes(),
        data,
    ]
    .concat()
}
fn group(kind: u16, label: &str, values: &[u8]) -> Vec<u8> {
    let name = name(label);
    block(
        kind,
        &[
            ((name.len() + 4) as u32).to_le_bytes().as_slice(),
            &name,
            values,
        ]
        .concat(),
    )
}
fn golden() -> Vec<u8> {
    [
        b"MR2\0".as_slice(),
        &block(
            0x80,
            &[name("Required"), name("ValidationRule"), name("Future")].concat(),
        ),
        &group(
            1,
            "Amount",
            &[
                value(0, 1, 0x80, &[0xff]),
                value(1, 10, 0, &[b'>', 0, b'0', 0]),
                value(2, 0x7f, 0x23, &[1, 2, 3]),
            ]
            .concat(),
        ),
        &group(2, "Amount", &value(0, 1, 0, &[0])),
    ]
    .concat()
}

#[test]
fn golden_properties_preserve_flags_unknown_values_and_separate_scopes() {
    let groups = decode(&golden(), &mut budget(), &|| false).unwrap();
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].kind(), PropertyGroupKind::Column);
    assert_eq!(groups[1].kind(), PropertyGroupKind::Index);
    let required = groups[0].get("required").unwrap();
    assert!(required.boolean().unwrap());
    assert_eq!(required.flags(), 0x80);
    assert_eq!(required.raw_value(), &[0xff]);
    assert_eq!(
        groups[0].get("ValidationRule").unwrap().text().unwrap(),
        ">0"
    );
    let unknown = groups[0].get("Future").unwrap();
    assert_eq!(unknown.native_type(), 0x7f);
    assert_eq!(unknown.flags(), 0x23);
    assert_eq!(unknown.raw_value(), &[1, 2, 3]);
    assert_eq!(unknown.boolean(), Err(InspectError::Unsupported));
    assert!(!groups[1].get("Required").unwrap().boolean().unwrap());
    assert!(groups[0].get("DefaultValue").is_none());
}

#[test]
fn four_byte_ui_booleans_are_preserved_without_semantic_coercion() {
    let bytes = [
        b"MR2\0".as_slice(),
        &block(0x80, &[name("TotalsRow"), name("HideNewField")].concat()),
        &group(
            0,
            "",
            &[value(0, 1, 0, &[0, 0, 0, 0]), value(1, 1, 0, &[0, 0, 0, 0])].concat(),
        ),
    ]
    .concat();
    assert_eq!(bytes.len(), 92);
    let original = bytes.clone();
    let groups = decode(&bytes, &mut budget(), &|| false).unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].kind(), PropertyGroupKind::Table);
    assert_eq!(groups[0].name(), "");
    for label in ["TotalsRow", "HideNewField"] {
        let property = groups[0].get(label).unwrap();
        assert_eq!(property.native_type(), 1);
        assert_eq!(property.flags(), 0);
        assert_eq!(property.raw_value(), &[0, 0, 0, 0]);
        assert_eq!(property.boolean(), Err(InspectError::Malformed));
    }
    assert_eq!(bytes, original);

    let mut truncated = bytes;
    truncated.pop();
    assert!(matches!(
        decode(&truncated, &mut budget(), &|| false),
        Err(InspectError::Truncated)
    ));
}

#[test]
fn opaque_boolean_payloads_do_not_qualify_required_constraints() {
    for payload in [vec![], vec![1, 2], vec![0, 0, 0, 0], vec![1, 0, 0, 0]] {
        let properties = [
            b"MR2\0".as_slice(),
            &block(0x80, &name("Required")),
            &group(1, "Score", &value(0, 1, 0, &payload)),
        ]
        .concat();
        let decoded = decode(&properties, &mut budget(), &|| false).unwrap();
        let required = decoded[0].get("Required").unwrap();
        assert_eq!(required.raw_value(), payload.as_slice());
        assert_eq!(required.boolean(), Err(InspectError::Malformed));
        let mut source = Cursor::new(database_with_properties(&properties));
        let original = source.get_ref().clone();
        let report = crate::check_table_constraints(
            &mut source,
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(report.rows(), 3);
        let score = report
            .fields()
            .iter()
            .find(|field| field.name() == "Score")
            .unwrap();
        assert_eq!(score.required(), crate::RuleCheckStatus::Unsupported);
        assert_eq!(source.get_ref(), &original);
    }

    let properties = [
        b"MR2\0".as_slice(),
        &block(0x80, &name("Required")),
        &group(1, "Score", &value(0, 1, 0, &[1])),
    ]
    .concat();
    let report = crate::check_table_constraints(
        &mut Cursor::new(database_with_properties(&properties)),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let score = report
        .fields()
        .iter()
        .find(|field| field.name() == "Score")
        .unwrap();
    assert_eq!(score.required(), crate::RuleCheckStatus::Checked);
}

fn database_with_properties(properties: &[u8]) -> Vec<u8> {
    let mut item = raw::record(10, 0, 1, 0, b"\xff\xfeItems");
    item.truncate(item.len() - 7);
    item[..2].copy_from_slice(&6_u16.to_le_bytes());
    let property_start = item.len() as u16;
    item.extend_from_slice(&(0x80000000 | properties.len() as u32).to_le_bytes());
    item.extend_from_slice(&[0; 8]);
    item.extend_from_slice(properties);
    item.extend_from_slice(&(item.len() as u16).to_le_bytes());
    item.extend_from_slice(&property_start.to_le_bytes());
    item.extend_from_slice(&16_u16.to_le_bytes());
    item.extend_from_slice(&2_u16.to_le_bytes());
    item.push(0x3f);

    let page = raw::data_page(
        2,
        &[
            (0, raw::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects")),
            (
                0,
                raw::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
            ),
            (0, item),
        ],
    );
    let mut bytes = crate::test_support::base::database();
    bytes[4 * 4096..5 * 4096].copy_from_slice(&page);
    bytes
}

#[test]
fn invalid_lengths_names_references_and_duplicates_do_not_fabricate_properties() {
    let names = block(0x80, &name("Required"));
    for invalid in [
        [b"MR2\0".as_slice(), &group(1, "X", &value(0, 1, 0, &[1]))].concat(),
        [b"MR2\0".as_slice(), &names, &names].concat(),
        [
            b"MR2\0".as_slice(),
            &names,
            &group(1, "X", &value(1, 1, 0, &[1])),
        ]
        .concat(),
        [
            b"MR2\0".as_slice(),
            &names,
            &group(
                1,
                "X",
                &[value(0, 1, 0, &[1]), value(0, 1, 0, &[0])].concat(),
            ),
        ]
        .concat(),
        [
            b"MR2\0".as_slice(),
            &names,
            &group(1, "", &value(0, 1, 0, &[1])),
        ]
        .concat(),
        [b"MR2\0".as_slice(), &block(0x80, &[2, 0, 0, 0xd8])].concat(),
        [b"MR2\0".as_slice(), &[0xff; 6]].concat(),
    ] {
        assert!(decode(&invalid, &mut budget(), &|| false).is_err());
    }
    let repeated = group(1, "X", &value(0, 1, 0, &[1]));
    assert!(
        decode(
            &[b"MR2\0".as_slice(), &names, &repeated, &repeated].concat(),
            &mut budget(),
            &|| false
        )
        .is_err()
    );
}

#[test]
fn work_and_allocations_are_bounded_and_cancellable() {
    let bytes = golden();
    assert!(matches!(
        decode(&bytes, &mut ResourceBudget::new(0, 0, 1000), &|| false),
        Err(InspectError::Limit(_))
    ));
    assert!(matches!(
        decode(&bytes, &mut ResourceBudget::new(1000, 0, 0), &|| false),
        Err(InspectError::Limit(_))
    ));
    assert!(matches!(
        decode(&bytes, &mut budget(), &|| true),
        Err(InspectError::Cancelled)
    ));
    let calls = std::cell::Cell::new(0);
    assert!(matches!(
        decode(&bytes, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            calls.get() > 5
        }),
        Err(InspectError::Cancelled)
    ));
    for end in 0..bytes.len() {
        let _ = decode(&bytes[..end], &mut budget(), &|| false);
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases:256, rng_seed:proptest::test_runner::RngSeed::Fixed(0x50524f5053), ..Default::default() })]
    #[test]
    fn arbitrary_property_streams_fail_or_decode_without_panicking(bytes in prop::collection::vec(any::<u8>(),0..4096)) {
        let _ = decode(&bytes, &mut budget(), &||false);
    }
}
