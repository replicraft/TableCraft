use super::*;
use crate::{NativeAddColumnIntent, NativeDefaultZeroAddColumnIntent};
use tablecraft_core::limits::Resource;

#[test]
fn closed_version_profile_and_required_tag_do_not_cross_decode() {
    let before = raw::source(Source::IdOnly);
    let wire = literal(&before, "Table1", "Score2");
    assert!(NativeAddColumnIntent::decode(&wire, &mut budget(), &|| false).is_err());
    assert!(NativeDefaultZeroAddColumnIntent::decode(&wire, &mut budget(), &|| false).is_err());
    for (version, profile) in [(b'1', 1_u16), (b'2', 2), (b'3', 3), (b'5', 5)] {
        let mut other = wire.clone();
        other[7] = version;
        other[12..14].copy_from_slice(&profile.to_le_bytes());
        assert!(matches!(
            NativeRepeatedDefaultZeroAddColumnIntent::decode(&other, &mut budget(), &|| false),
            Err(RepeatedDefaultZeroAddColumnIntentError::InvalidRecord)
        ));
    }
    for tag in [0_u16, 2, u16::MAX] {
        let mut other = wire.clone();
        other[14..16].copy_from_slice(&tag.to_le_bytes());
        assert!(matches!(
            NativeRepeatedDefaultZeroAddColumnIntent::decode(&other, &mut budget(), &|| false),
            Err(RepeatedDefaultZeroAddColumnIntentError::InvalidRecord)
        ));
    }
    let parsed = decode(&wire);
    assert_eq!(
        parsed.required_property(),
        CreatedDefaultZeroColumnRequired::StoredFalse
    );
    assert_eq!(parsed.encode(&mut budget(), &|| false).unwrap(), wire);
}

#[test]
fn malformed_framing_and_both_names_refuse_before_owned_name_allocation() {
    let before = raw::source(Source::IdOnly);
    let wire = literal(&before, "Table1", "Score2");
    let mut malformed = (0..wire.len())
        .map(|end| wire[..end].to_vec())
        .collect::<Vec<_>>();
    for offset in [0, 8, 12, 14, 56, 58, 60] {
        let mut bytes = wire.clone();
        bytes[offset] = 0xff;
        malformed.push(bytes);
    }
    let mut zero_length = wire.clone();
    zero_length[16..24].fill(0);
    malformed.push(zero_length);
    for (table, column) in [("", "Score2"), ("Table1", ""), ("Table1", "bad\0name")] {
        malformed.push(literal(&before, table, column));
    }
    malformed.push(literal(&before, &"a".repeat(65), "Score2"));
    malformed.push(literal(&before, "Table1", &"😀".repeat(33)));
    let mut trailing = wire;
    trailing.push(0);
    malformed.push(trailing);
    for bytes in malformed {
        let mut no_names = ResourceBudget::new(0, 0, 500_000_000);
        let error =
            NativeRepeatedDefaultZeroAddColumnIntent::decode(&bytes, &mut no_names, &|| false)
                .err()
                .expect("invalid borrowed framing/names must refuse");
        assert!(!matches!(
            error,
            RepeatedDefaultZeroAddColumnIntentError::Inspect(InspectError::Limit(_))
        ));
        assert_eq!(no_names.remaining(Resource::Bytes), 0);
    }
    assert!(matches!(
        NativeRepeatedDefaultZeroAddColumnIntent::decode(
            &literal(&before, "Table1", "Score2"),
            &mut budget(),
            &|| true
        ),
        Err(RepeatedDefaultZeroAddColumnIntentError::Inspect(
            InspectError::Cancelled
        ))
    ));
}
