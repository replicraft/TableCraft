use super::*;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 100, 1_000_000)
}

#[test]
fn null_empty_inline_single_and_chained_descriptors_match_literal_wire() {
    assert_eq!(
        Encoded::Null
            .descriptor(StorageChoice::Inline, None, &mut budget(), &|| false)
            .unwrap(),
        Vec::<u8>::new()
    );
    for length in [0_usize, 1, 64, 65, 4076, 4077] {
        let encoded = Encoded::Present(vec![0x61; length]);
        let choice = encoded.preferred();
        let first = if length <= 64 {
            None
        } else {
            Some(0x0000_ff00)
        };
        let actual = encoded
            .descriptor(choice, first, &mut budget(), &|| false)
            .unwrap();
        let tag = if length <= 64 {
            0x8000_0000
        } else if length <= 4076 {
            0x4000_0000
        } else {
            0
        };
        let mut expected = (tag | length as u32).to_le_bytes().to_vec();
        expected.extend_from_slice(&first.unwrap_or(0).to_le_bytes());
        expected.extend_from_slice(&[0, 0, 0, 0]);
        if length <= 64 {
            expected.extend_from_slice(&vec![0x61; length]);
        }
        assert_eq!(actual, expected);
    }
}

#[test]
fn full_chain_rows_include_links_and_terminal_payload_is_not_padded() {
    let encoded = Encoded::Present((0..4077).map(|index| (index % 251) as u8).collect());
    let chunks = encoded
        .chunks(StorageChoice::Chained, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].payload, 0..4072);
    assert_eq!(chunks[1].payload, 4072..4077);
    let first = encoded
        .body(&chunks[0], 0x0001_0000, &mut budget(), &|| false)
        .unwrap();
    let last = encoded
        .body(&chunks[1], 0, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(first.len(), 4076);
    assert_eq!(first[..4], [0, 0, 1, 0]);
    assert_eq!(last.len(), 9);
    assert_eq!(last[..4], [0, 0, 0, 0]);
    assert_eq!(
        first[4..],
        (0..4072)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        last[4..],
        (4072..4077)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>()
    );
}

#[test]
fn small_nonempty_external_choice_is_explicit_and_empty_external_is_invalid() {
    let encoded = Encoded::Present(vec![0x42; 64]);
    let chunks = encoded
        .chunks(StorageChoice::Single, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(chunks.len(), 1);
    let descriptor = encoded
        .descriptor(
            StorageChoice::Single,
            Some(0x0000_ff00),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(descriptor, [64, 0, 0, 64, 0, 255, 0, 0, 0, 0, 0, 0]);
    assert_eq!(
        encoded
            .body(&chunks[0], 0, &mut budget(), &|| false)
            .unwrap(),
        vec![0x42; 64]
    );
    assert!(
        Encoded::Present(Vec::new())
            .chunks(StorageChoice::Single, &mut budget(), &|| false)
            .is_err()
    );
    assert!(
        encoded
            .descriptor(
                StorageChoice::Inline,
                Some(0x0000_ff00),
                &mut budget(),
                &|| false
            )
            .is_err()
    );
}
