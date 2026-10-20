use super::*;
#[test]
fn supported_pages_decode_independent_goldens_without_bom_or_replacement() {
    for (id, byte, expected) in [
        (1252, 0x80, '€'),
        (1252, 0x81, '\u{81}'),
        (1252, 0xe9, 'é'),
        (1251, 0xc0, 'А'),
        (1251, 0x98, '\u{98}'),
        (1250, 0xa5, 'Ą'),
        (1253, 0xe1, 'α'),
        (1254, 0xd0, 'Ğ'),
        (1255, 0xe0, 'א'),
        (1256, 0xc7, 'ا'),
        (1257, 0xc0, 'Ą'),
        (1258, 0xcc, '\u{300}'),
        (874, 0xa1, 'ก'),
        (866, 0x80, 'А'),
    ] {
        let page = CharacterCodePage::from_windows_id(id).unwrap();
        assert_eq!(page.windows_id(), id);
        assert_eq!(page.decode_byte(byte), Ok(expected));
    }
    assert_eq!(
        CharacterCodePage::from_windows_id(1253)
            .unwrap()
            .decode_byte(0xaa),
        Err(EvalError::InvalidCharacterCode)
    );
    for id in [0, 1, 437, 932, 1200, 65001, u16::MAX] {
        assert_eq!(
            CharacterCodePage::from_windows_id(id),
            Err(EvalError::UnsupportedCharacterCodePage)
        );
    }
}
#[test]
fn every_supported_page_preserves_the_complete_ascii_byte_profile() {
    for id in [
        866, 874, 1250, 1251, 1252, 1253, 1254, 1255, 1256, 1257, 1258,
    ] {
        let page = CharacterCodePage::from_windows_id(id).unwrap();
        for byte in 0u8..=127 {
            assert_eq!(page.decode_byte(byte), Ok(char::from(byte)));
        }
    }
}

#[test]
fn mapped_single_byte_characters_encode_exactly_and_unmappable_text_fails() {
    for id in [
        866, 874, 1250, 1251, 1252, 1253, 1254, 1255, 1256, 1257, 1258,
    ] {
        let page = CharacterCodePage::from_windows_id(id).unwrap();
        for byte in 0u8..=255 {
            if let Ok(character) = page.decode_byte(byte) {
                let encoded = page.encode_character(character).unwrap();
                assert_eq!(page.decode_byte(encoded), Ok(character));
            }
        }
    }
    let page = CharacterCodePage::from_windows_id(1252).unwrap();
    assert_eq!(page.encode_character('€'), Ok(128));
    assert_eq!(
        page.encode_character('中'),
        Err(EvalError::UnmappableCharacter)
    );
    assert_eq!(
        page.encode_character('😀'),
        Err(EvalError::UnmappableCharacter)
    );
}
