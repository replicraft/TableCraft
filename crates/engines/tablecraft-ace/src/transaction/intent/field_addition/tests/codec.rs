use super::*;
use crate::{
    NativeAddColumnIntent, NativeDefaultZeroAddColumnIntent,
    NativeRepeatedDefaultZeroAddColumnIntent,
};
use tablecraft_core::limits::Resource;

#[test]
fn literal_target_records_encode_exactly_and_never_cross_decode_legacy_versions() {
    let before = raw::source(raw::SourceCase::IdOnly);
    for target in [
        FieldAdditionTarget::NumberLongZero,
        text(1, false),
        text(80, true),
        text(255, true),
    ] {
        let wire = literal(&before, "Table1", "Label", target);
        assert_eq!(&wire[..14], b"TCAC0005K\0\0\0\x05\0");
        let parsed = decode(&wire);
        assert_eq!(parsed.table(), "Table1");
        assert_eq!(parsed.column(), "Label");
        assert_eq!(parsed.target(), target);
        assert_eq!(parsed.encode(&mut budget(), &|| false).unwrap(), wire);
        assert!(NativeAddColumnIntent::decode(&wire, &mut budget(), &|| false).is_err());
        assert!(NativeDefaultZeroAddColumnIntent::decode(&wire, &mut budget(), &|| false).is_err());
        assert!(
            NativeRepeatedDefaultZeroAddColumnIntent::decode(&wire, &mut budget(), &|| false)
                .is_err()
        );
    }
    let wire = literal(&before, "Table1", "Label", text(255, true));
    for version in b'1'..=b'4' {
        let mut other = wire.clone();
        other[7] = version;
        other[12..14].copy_from_slice(&u16::from(version - b'0').to_le_bytes());
        assert!(matches!(
            NativeFieldAdditionIntent::decode(&other, &mut budget(), &|| false),
            Err(FieldAdditionIntentError::InvalidRecord)
        ));
    }
}

#[test]
fn target_tags_widths_and_compression_are_closed_before_name_allocation() {
    let before = raw::source(raw::SourceCase::IdOnly);
    let base = literal(&before, "Table1", "Label", text(255, true));
    for (tag, width, compression) in [
        (0_u16, 255_u16, 1_u16),
        (3, 255, 1),
        (1, 1, 0),
        (1, 0, 1),
        (2, 0, 0),
        (2, 256, 1),
        (2, 80, 2),
    ] {
        let mut wire = base.clone();
        wire[14..16].copy_from_slice(&tag.to_le_bytes());
        wire[16..18].copy_from_slice(&width.to_le_bytes());
        wire[18..20].copy_from_slice(&compression.to_le_bytes());
        let mut borrowed_only = ResourceBudget::new(0, 0, 500_000_000);
        assert!(matches!(
            NativeFieldAdditionIntent::decode(&wire, &mut borrowed_only, &|| false),
            Err(FieldAdditionIntentError::InvalidRecord)
        ));
        assert_eq!(borrowed_only.remaining(Resource::Bytes), 0);
    }
}

#[test]
fn malformed_framing_names_and_cancellation_remain_typed() {
    let before = raw::source(raw::SourceCase::IdOnly);
    let wire = literal(&before, "Table1", "Label", text(255, true));
    let mut malformed = (0..wire.len())
        .map(|end| wire[..end].to_vec())
        .collect::<Vec<_>>();
    for offset in [0, 8, 12, 60, 62, 64] {
        let mut bytes = wire.clone();
        bytes[offset] = 0xff;
        malformed.push(bytes);
    }
    for (table, column) in [("", "Label"), ("Table1", ""), ("Table1", "bad\0name")] {
        malformed.push(literal(&before, table, column, text(255, true)));
    }
    malformed.push(literal(&before, &"a".repeat(65), "Label", text(255, true)));
    malformed.push(literal(
        &before,
        "Table1",
        &"😀".repeat(33),
        text(255, true),
    ));
    let mut trailing = wire.clone();
    trailing.push(0);
    malformed.push(trailing);
    for bytes in malformed {
        let error = NativeFieldAdditionIntent::decode(
            &bytes,
            &mut ResourceBudget::new(0, 0, 500_000_000),
            &|| false,
        )
        .err()
        .unwrap();
        assert!(!matches!(
            error,
            FieldAdditionIntentError::Inspect(InspectError::Limit(_))
        ));
    }
    assert!(matches!(
        NativeFieldAdditionIntent::decode(&wire, &mut budget(), &|| true),
        Err(FieldAdditionIntentError::Inspect(InspectError::Cancelled))
    ));
}
