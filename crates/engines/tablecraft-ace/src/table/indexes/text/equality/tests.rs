use super::*;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1024 * 1024, 0, 100_000)
}

#[test]
fn owned_prefixes_retain_independent_goldens_and_validate_encoded_boundaries() {
    for (version, expected) in [(0, vec![0x4a]), (1, vec![0x0e, 0x02])] {
        let order = NativeSortOrder::from_native_bytes([9, 4, 0, version]);
        let prefix =
            GeneralEqualityPrefix::from_text(order, "A  ", &mut budget(), &|| false).unwrap();
        assert_eq!(prefix.order(), order);
        assert_eq!(prefix.as_bytes(), expected);
        assert_eq!(
            GeneralEqualityPrefix::from_encoded(order, prefix.as_bytes()).unwrap(),
            prefix
        );
        assert_eq!(
            GeneralEqualityPrefix::from_encoded(order, &[0; 33]),
            Err(InspectError::StructureLimit)
        );
        assert_eq!(
            GeneralEqualityPrefix::from_text(order, &"x".repeat(256), &mut budget(), &|| false),
            Err(InspectError::StructureLimit)
        );
        let long =
            GeneralEqualityPrefix::from_text(order, &"a".repeat(255), &mut budget(), &|| false)
                .unwrap();
        assert_eq!(long.as_bytes().len(), 32);
    }
    let unknown = NativeSortOrder::from_native_bytes([9, 4, 1, 1]);
    assert_eq!(
        GeneralEqualityPrefix::from_encoded(unknown, &[]),
        Err(InspectError::Unsupported)
    );
}
