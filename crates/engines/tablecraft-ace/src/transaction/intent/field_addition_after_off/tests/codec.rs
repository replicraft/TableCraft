use super::*;
use crate::NativeFieldAdditionIntent;

#[test]
fn exact_literal_records_are_distinct_from_every_legacy_version() {
    let before = raw::source(raw::SourceCase::Number);
    for target in [FieldAdditionTarget::NumberLongZero, text()] {
        let wire = literal(&before, "Table1", "AfterOff", target);
        assert_eq!(&wire[..14], b"TCAC0006N\0\0\0\x06\0");
        let parsed = decode(&wire);
        assert_eq!(parsed.table(), "Table1");
        assert_eq!(parsed.column(), "AfterOff");
        assert_eq!(parsed.target(), target);
        assert_eq!(parsed.encode(&mut budget(), &|| false).unwrap(), wire);
        assert!(NativeFieldAdditionIntent::decode(&wire, &mut budget(), &|| false).is_err());
        for version in b'1'..=b'5' {
            let mut old = wire.clone();
            old[7] = version;
            old[12..14].copy_from_slice(&u16::from(version - b'0').to_le_bytes());
            assert!(matches!(
                NativeFieldAdditionAfterOffIntent::decode(&old, &mut budget(), &|| false),
                Err(FieldAdditionAfterOffIntentError::InvalidRecord)
            ));
        }
    }
}

#[test]
fn width_compression_policy_and_namespace_substitution_refuse_before_allocation() {
    let before = raw::source(raw::SourceCase::Number);
    for (tag, width, compression) in [
        (0_u16, 255_u16, 1_u16),
        (3, 255, 1),
        (1, 1, 0),
        (1, 0, 1),
        (2, 80, 1),
        (2, 255, 0),
        (2, 255, 2),
    ] {
        let mut wire = literal(&before, "Table1", "LabelOff", text());
        wire[14..16].copy_from_slice(&tag.to_le_bytes());
        wire[16..18].copy_from_slice(&width.to_le_bytes());
        wire[18..20].copy_from_slice(&compression.to_le_bytes());
        assert!(matches!(
            NativeFieldAdditionAfterOffIntent::decode(
                &wire,
                &mut ResourceBudget::new(0, 0, 5_000_000),
                &|| false
            ),
            Err(FieldAdditionAfterOffIntentError::InvalidRecord)
        ));
    }
    for (table, column) in [("Tāble1", "New"), ("Table1", "Σ")] {
        assert!(matches!(
            NativeFieldAdditionAfterOffIntent::decode(
                &literal(&before, table, column, text()),
                &mut budget(),
                &|| false
            ),
            Err(FieldAdditionAfterOffIntentError::InvalidRecord)
        ));
    }
}

#[test]
fn framing_names_and_cancellation_are_closed() {
    let before = raw::source(raw::SourceCase::Number);
    let wire = literal(
        &before,
        "Table1",
        "AfterOff",
        FieldAdditionTarget::NumberLongZero,
    );
    for end in 0..wire.len() {
        assert!(
            NativeFieldAdditionAfterOffIntent::decode(&wire[..end], &mut budget(), &|| false)
                .is_err()
        );
    }
    for at in [0, 8, 12, 60, 62, 64] {
        let mut bad = wire.clone();
        bad[at] = 255;
        assert!(NativeFieldAdditionAfterOffIntent::decode(&bad, &mut budget(), &|| false).is_err());
    }
    for (table, column) in [("", "New"), ("Table1", ""), ("Table1", "bad\0name")] {
        assert!(
            NativeFieldAdditionAfterOffIntent::decode(
                &literal(&before, table, column, text()),
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
    let mut extra = wire.clone();
    extra.push(0);
    assert!(NativeFieldAdditionAfterOffIntent::decode(&extra, &mut budget(), &|| false).is_err());
    assert!(matches!(
        NativeFieldAdditionAfterOffIntent::decode(&wire, &mut budget(), &|| true),
        Err(FieldAdditionAfterOffIntentError::Inspect(
            InspectError::Cancelled
        ))
    ));
}
