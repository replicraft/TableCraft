use super::*;
use crate::{
    comparison::LocaleId,
    functions::text::strconv::{LocaleProfile, StringLocaleProfile},
};

#[derive(Default)]
struct Output {
    bytes: Vec<u8>,
    text: String,
    steps: usize,
    stop: Option<usize>,
    limit: Option<usize>,
}

impl Output {
    fn step(&mut self) -> Result<(), EvalError> {
        if self.stop.is_some_and(|stop| self.steps >= stop) {
            return Err(EvalError::Cancelled);
        }
        self.steps += 1;
        Ok(())
    }
}

impl ByteOutput for Output {
    fn step(&mut self) -> Result<(), EvalError> {
        self.step()
    }

    fn append(&mut self, bytes: &[u8]) -> Result<(), EvalError> {
        if self
            .limit
            .is_some_and(|limit| self.bytes.len() + bytes.len() > limit)
        {
            return Err(EvalError::TextLimit);
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
}

impl TextOutput for Output {
    fn step(&mut self) -> Result<(), EvalError> {
        self.step()
    }

    fn append(&mut self, text: &str) -> Result<(), EvalError> {
        if self
            .limit
            .is_some_and(|limit| self.text.len() + text.len() > limit)
        {
            return Err(EvalError::TextLimit);
        }
        self.text.push_str(text);
        Ok(())
    }
}

fn japanese() -> StringCodePage {
    StringCodePage::from_windows_id(932).unwrap()
}

#[test]
fn exact_japanese_mapping_and_canonical_duplicate_spelling_are_explicit() {
    for (text, bytes) in [
        ("日本", &[0x93, 0xfa, 0x96, 0x7b][..]),
        ("ｶﾞ", &[0xb6, 0xde][..]),
        ("あア", &[0x82, 0xa0, 0x83, 0x41][..]),
        ("A\0", &[0x41, 0x00][..]),
        ("ⅰ", &[0xfa, 0x40][..]),
        ("－", &[0x81, 0x7c][..]),
    ] {
        let mut encoded = Output::default();
        japanese().encode(text, &mut encoded).unwrap();
        assert_eq!(encoded.bytes, bytes);

        let mut decoded = Output::default();
        japanese().decode(bytes, &mut decoded).unwrap();
        assert_eq!(decoded.text, text);
    }

    let mut alternate = Output::default();
    japanese().decode(&[0xee, 0xef], &mut alternate).unwrap();
    assert_eq!(alternate.text, "ⅰ");
    let mut private_use = Output::default();
    japanese().decode(&[0xf0, 0x40], &mut private_use).unwrap();
    assert_eq!(private_use.text, "\u{e000}");
}

#[test]
fn all_ascii_and_halfwidth_single_bytes_match_independent_scalar_ranges() {
    for byte in 0_u8..=127 {
        let mut encoded = Output::default();
        japanese()
            .encode(&char::from(byte).to_string(), &mut encoded)
            .unwrap();
        assert_eq!(encoded.bytes, [byte]);
        let mut decoded = Output::default();
        japanese().decode(&[byte], &mut decoded).unwrap();
        assert_eq!(decoded.text, char::from(byte).to_string());
    }
    for byte in 0xa1_u8..=0xdf {
        let character = char::from_u32(0xff61 + u32::from(byte - 0xa1)).unwrap();
        let mut decoded = Output::default();
        japanese().decode(&[byte], &mut decoded).unwrap();
        assert_eq!(decoded.text, character.to_string());
        let mut encoded = Output::default();
        japanese()
            .encode(&character.to_string(), &mut encoded)
            .unwrap();
        assert_eq!(encoded.bytes, [byte]);
    }
}

#[test]
fn aliases_unmappable_scalars_and_incomplete_tokens_never_replace_or_lose_input() {
    for text in ["¥", "‾", "−", "😀", "\u{e000}"] {
        assert_eq!(
            japanese().encode(text, &mut Output::default()),
            Err(EvalError::UnmappableCharacter)
        );
    }
    for bytes in [
        &[0x81][..],
        &[0xe0][..],
        &[0x81, 0x7f][..],
        &[0x81, 0xff][..],
        &[0xa0][..],
        &[0xfd][..],
        &[0xfe][..],
        &[0xff][..],
    ] {
        assert_eq!(
            japanese().decode(bytes, &mut Output::default()),
            Err(EvalError::InvalidCharacterCode)
        );
    }
    let mut extension = Output::default();
    japanese().decode(&[0x80], &mut extension).unwrap();
    assert_eq!(extension.text, "\u{80}");
}

#[test]
fn complete_locale_api_preserves_old_single_byte_contracts_and_identity_table() {
    for id in [
        1025, 1029, 1031, 1032, 1033, 1036, 1037, 1043, 1049, 1054, 1055, 1061, 1063, 1066,
    ] {
        let id = LocaleId::new(id).unwrap();
        let old = LocaleProfile::from_lcid(id).unwrap();
        let complete = StringLocaleProfile::from_lcid(id).unwrap();
        assert_eq!(old.case_locale(), complete.case_locale());
        assert_eq!(
            old.code_page().windows_id(),
            complete.code_page().windows_id()
        );
    }
    let id = LocaleId::new(1041).unwrap();
    assert_eq!(
        StringLocaleProfile::from_lcid(id)
            .unwrap()
            .code_page()
            .windows_id(),
        932
    );
    assert_eq!(
        LocaleProfile::from_lcid(id),
        Err(EvalError::UnsupportedExpression)
    );
    assert_eq!(
        CharacterCodePage::from_windows_id(932),
        Err(EvalError::UnsupportedCharacterCodePage)
    );
    for id in [0, 65001, 936, 949, 950] {
        assert_eq!(
            StringCodePage::from_windows_id(id),
            Err(EvalError::UnsupportedCharacterCodePage)
        );
    }
    assert!(StringLocaleProfile::from_lcid(LocaleId::new(0x10411).unwrap()).is_err());
}

#[test]
fn every_codec_work_checkpoint_and_output_limit_preserves_sink_failure() {
    let mut encoded = Output::default();
    japanese().encode("日本Aｶﾞ", &mut encoded).unwrap();
    for stop in 0..encoded.steps {
        let mut output = Output {
            stop: Some(stop),
            ..Default::default()
        };
        assert_eq!(
            japanese().encode("日本Aｶﾞ", &mut output),
            Err(EvalError::Cancelled)
        );
    }
    let mut decoded = Output::default();
    japanese().decode(&encoded.bytes, &mut decoded).unwrap();
    for stop in 0..decoded.steps {
        let mut output = Output {
            stop: Some(stop),
            ..Default::default()
        };
        assert_eq!(
            japanese().decode(&encoded.bytes, &mut output),
            Err(EvalError::Cancelled)
        );
    }
    let mut encoded_limit = Output {
        limit: Some(encoded.bytes.len() - 1),
        ..Default::default()
    };
    assert_eq!(
        japanese().encode("日本Aｶﾞ", &mut encoded_limit),
        Err(EvalError::TextLimit)
    );
    let mut decoded_limit = Output {
        limit: Some(decoded.text.len() - 1),
        ..Default::default()
    };
    assert_eq!(
        japanese().decode(&encoded.bytes, &mut decoded_limit),
        Err(EvalError::TextLimit)
    );
}
