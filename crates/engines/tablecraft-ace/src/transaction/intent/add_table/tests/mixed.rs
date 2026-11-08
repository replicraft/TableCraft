//! Independent mixed-ledger framing, key placement and target-capacity boundaries.
use super::*;

fn manual_definition(keyed: bool) -> FirstTableDefinition {
    let id = CreateColumn::new("Id", ScalarType::Int32);
    let id = if keyed {
        id.with_primary_key("PrimaryId").unwrap()
    } else {
        id
    };
    FirstTableDefinition::capture_manual(
        "More",
        &[id, CreateColumn::short_text("Name", 20).unwrap()],
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn manual_template(keyed: bool) -> Vec<u8> {
    let mut nested = template(1);
    nested[..8].copy_from_slice(b"TCNT0006");
    nested[12..14].copy_from_slice(&6_u16.to_le_bytes());
    if keyed {
        nested[62..64].copy_from_slice(&0_u16.to_le_bytes());
        nested[79] = 4;
    } else {
        nested[58..60].copy_from_slice(&0_u16.to_le_bytes());
        nested[62..64].copy_from_slice(&65535_u16.to_le_bytes());
        nested.drain(68..77);
        nested[70] = 4;
    }
    let length = nested.len() as u32;
    nested[8..12].copy_from_slice(&length.to_le_bytes());
    nested
}

fn mixed_literal(before: &[u8], nested: &[u8]) -> Vec<u8> {
    let mut raw = literal(before, 1);
    raw.truncate(64);
    raw[..8].copy_from_slice(b"TCAT0005");
    raw[12..14].copy_from_slice(&5_u16.to_le_bytes());
    raw[16..24].copy_from_slice(&(before.len() as u64).to_le_bytes());
    raw[56..60].copy_from_slice(&(nested.len() as u32).to_le_bytes());
    raw.extend(nested);
    let length = raw.len() as u32;
    raw[8..12].copy_from_slice(&length.to_le_bytes());
    raw
}

#[test]
fn mixed_v5_retains_exact_manual_identity_and_independent_target_capacity_bounds() {
    for keyed in [false, true] {
        for pages in [9, 10, 13, 61, 62] {
            let before = vec![0; pages * 4096];
            let result = NativeAddTableIntent::new_mixed(
                manual_definition(keyed),
                &before,
                &mut budget(),
                &|| false,
            );
            if keyed && pages == 62 {
                assert!(matches!(result, Err(AddTableIntentError::BeforeMismatch)));
                continue;
            }
            let raw = result.unwrap().encode(&mut budget(), &|| false).unwrap();
            assert_eq!(raw, mixed_literal(&before, &manual_template(keyed)));
            let decoded = NativeAddTableIntent::decode(&raw, &mut budget(), &|| false).unwrap();
            assert!(decoded.definition.is_manual());
            assert_eq!(decoded.definition.primary_position(), keyed.then_some(0));
            decoded
                .verify_before(&before, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), raw);
        }
    }
    for length in [
        0,
        8 * 4096,
        9 * 4096 - 1,
        9 * 4096 + 1,
        63 * 4096,
        64 * 4096,
    ] {
        let raw = mixed_literal(&vec![0; length], &manual_template(false));
        let mut no_ownership = ResourceBudget::new(0, 0, 1_000_000);
        assert!(matches!(
            NativeAddTableIntent::decode(&raw, &mut no_ownership, &|| false),
            Err(AddTableIntentError::InvalidRecord)
        ));
        assert_eq!(no_ownership.remaining(Resource::Bytes), 0);
    }
}

#[test]
fn mixed_v5_pins_all_six_templates_without_promoting_legacy_outer_languages() {
    let before = canonical_add_table::source().before;
    for version in 1..=4 {
        let raw =
            NativeAddTableIntent::new_mixed(definition(version), &before, &mut budget(), &|| false)
                .unwrap()
                .encode(&mut budget(), &|| false)
                .unwrap();
        assert_eq!(raw, mixed_literal(&before, &template(version)));
        let decoded = NativeAddTableIntent::decode(&raw, &mut budget(), &|| false).unwrap();
        assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), raw);
    }
    let raw =
        NativeAddTableIntent::new_mixed(described_definition(""), &before, &mut budget(), &|| {
            false
        })
        .unwrap()
        .encode(&mut budget(), &|| false)
        .unwrap();
    assert_eq!(&raw[..8], b"TCAT0005");
    assert_eq!(&raw[64..72], b"TCNT0005");
    let decoded = NativeAddTableIntent::decode(&raw, &mut budget(), &|| false).unwrap();
    assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), raw);

    let valid = mixed_literal(&before, &manual_template(false));
    for case in 0..8 {
        let mut raw = valid.clone();
        match case {
            0 => raw[7] = b'6',
            1 => raw[12] = 4,
            2 => raw[14] = 1,
            3 => raw[60] = 1,
            4 => raw[71] = b'7',
            5 => raw[76] = 5,
            6 => raw[78] = 1,
            7 => raw.push(0),
            _ => unreachable!(),
        }
        let mut no_ownership = ResourceBudget::new(0, 0, 1_000_000);
        assert!(
            matches!(
                NativeAddTableIntent::decode(&raw, &mut no_ownership, &|| false),
                Err(AddTableIntentError::UnsupportedVersion
                    | AddTableIntentError::UnsupportedProfile
                    | AddTableIntentError::InvalidRecord)
            ),
            "case {case}"
        );
        assert_eq!(no_ownership.remaining(Resource::Bytes), 0);
    }
    let mut none = ResourceBudget::new(0, 0, 1_000_000);
    assert!(NativeAddTableIntent::decode(&valid, &mut none, &|| false).is_err());
    assert!(matches!(
        NativeAddTableIntent::decode(&valid, &mut budget(), &|| true),
        Err(AddTableIntentError::Inspect(InspectError::Cancelled))
    ));
}
