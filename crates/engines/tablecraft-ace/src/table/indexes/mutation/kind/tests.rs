use super::*;

#[test]
fn literal_guid_field_and_segment_goldens_keep_distinct_byte_orders() {
    let canonical = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
    let stored = [3, 2, 1, 0, 5, 4, 7, 6, 8, 9, 10, 11, 12, 13, 14, 15];
    assert!(MutationKey::Guid.stored(&stored).unwrap() == MixedValue::Guid(canonical));
    assert!(MutationKey::Guid.stored(&stored[..15]).is_err());
    for (descending, expected) in [
        (
            false,
            [
                127, 0, 1, 2, 3, 4, 5, 6, 7, 9, 8, 9, 10, 11, 12, 13, 14, 15, 8,
            ],
        ),
        (
            true,
            [
                128, 255, 254, 253, 252, 251, 250, 249, 248, 9, 247, 246, 245, 244, 243, 242, 241,
                240, 247,
            ],
        ),
    ] {
        let mut bytes = [0; 19];
        assert_eq!(
            MutationKey::Guid
                .encode(Some(MixedValue::Guid(canonical)), descending, &mut bytes)
                .unwrap(),
            expected.len()
        );
        assert_eq!(bytes, expected);
        assert!(
            MutationKey::Guid
                .encode(
                    Some(MixedValue::Guid(canonical)),
                    descending,
                    &mut bytes[..18]
                )
                .is_err()
        );
    }
    let mut bytes = [0; 1];
    assert_eq!(MutationKey::Guid.encode(None, true, &mut bytes).unwrap(), 1);
    assert_eq!(bytes, [255]);
    assert!(
        MutationKey::Guid
            .encode(Some(MixedValue::Numeric(1)), false, &mut [0; 19])
            .is_err()
    );
    assert!(
        MutationKey::new(ScalarType::Int64)
            .unwrap()
            .encode(Some(MixedValue::Guid(canonical)), false, &mut [0; 19])
            .is_err()
    );
}
