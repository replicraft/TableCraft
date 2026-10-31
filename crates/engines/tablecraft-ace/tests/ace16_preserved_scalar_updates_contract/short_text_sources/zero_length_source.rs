//! Separate one-byte derivative of the original framed Short_Text declaration.
fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn integer(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}

fn text(bytes: &[u8]) -> String {
    let (units, remainder) = bytes.as_chunks::<2>();
    assert!(remainder.is_empty());
    String::from_utf16(
        &units
            .iter()
            .map(|unit| u16::from_le_bytes(*unit))
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

pub(crate) fn disallow_empty(before: &[u8], original_properties: &[u8]) -> (Vec<u8>, usize) {
    let locations = before
        .windows(original_properties.len())
        .enumerate()
        .filter_map(|(at, bytes)| (bytes == original_properties).then_some(at))
        .collect::<Vec<_>>();
    assert_eq!(locations.len(), 1);
    let location = locations[0];
    assert!(location >= 12);
    assert_eq!(
        &before[location - 12..location - 8],
        &(0x8000_0000_u32 | u32::try_from(original_properties.len()).unwrap()).to_le_bytes()
    );
    assert_eq!(&before[location - 8..location], &[0; 8]);
    assert_eq!(&original_properties[..4], b"MR2\0");
    assert_eq!(word(original_properties, 8), 0x80);
    let names_end = 4 + integer(original_properties, 4);
    let mut at = 10;
    let mut names = Vec::new();
    while at < names_end {
        let length = word(original_properties, at);
        at += 2;
        assert!(at + length <= names_end);
        names.push(text(&original_properties[at..at + length]));
        at += length;
    }
    assert_eq!(at, names_end);
    let property_name = names
        .iter()
        .position(|name| name == "AllowZeroLength")
        .unwrap();
    let mut selected = None;
    while at < original_properties.len() {
        let end = at + integer(original_properties, at);
        assert!(end > at + 6 && end <= original_properties.len());
        let header = integer(original_properties, at + 6);
        let name_length = word(original_properties, at + 10);
        assert_eq!(header, 6 + name_length);
        let name_end = at + 12 + name_length;
        assert!(name_end <= end);
        if word(original_properties, at + 4) == 1
            && text(&original_properties[at + 12..name_end]) == "Short_Text"
        {
            let mut property = at + 6 + header;
            while property < end {
                let length = word(original_properties, property);
                assert!(length >= 8 && property + length <= end);
                if word(original_properties, property + 4) == property_name {
                    assert!(selected.replace(property + 8).is_none());
                    assert_eq!(&original_properties[property + 2..property + 4], &[1, 1]);
                    assert_eq!(word(original_properties, property + 6), 1);
                    assert_eq!(length, 9);
                    assert_eq!(original_properties[property + 8], 1);
                }
                property += length;
            }
            assert_eq!(property, end);
        }
        at = end;
    }
    assert_eq!(at, original_properties.len());
    let patch = location + selected.unwrap();
    let mut derivative = before.to_vec();
    derivative[patch] = 0;
    assert_eq!(&derivative[..patch], &before[..patch]);
    assert_eq!(&derivative[patch + 1..], &before[patch + 1..]);
    (derivative, patch)
}
