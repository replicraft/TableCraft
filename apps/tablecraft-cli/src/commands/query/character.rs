//! Validated explicit character-code policy; no ambient system encoding lookup.
use tablecraft_expressions::functions::text::code_page::CharacterCodePage;

pub(super) fn parse_code_page(text: &str) -> Result<CharacterCodePage, String> {
    text.parse::<u16>()
        .ok()
        .and_then(|id| CharacterCodePage::from_windows_id(id).ok())
        .ok_or_else(|| {
            "expected a supported single-byte code page: 866, 874, or 1250..=1258".to_owned()
        })
}
