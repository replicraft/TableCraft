//! Bundled UI fonts, independent of database sort and regional value policies.
mod inventory;

pub use inventory::{FontFamilies, FontInventoryError};

use crate::Locale;
use egui::{Context, FontData, FontDefinitions, FontFamily, Id, Ui};

pub(crate) const BOLD_FAMILY: &str = "Selawik Bold";

const SELAWIK: &str = "Selawik";
const NOTO_JAPANESE: &str = "Noto Sans JP";
const NOTO_KOREAN: &str = "Noto Sans KR";
const NOTO_SIMPLIFIED_CHINESE: &str = "Noto Sans SC";
const NOTO_EMOJI: &str = "Noto Emoji";
const EGUI_EMOJI_FALLBACKS: [&str; 2] = ["NotoEmoji-Regular", "emoji-icon-font"];

/// Install Selawik, monochrome emoji and regional CJK fallbacks once per locale change.
/// Font selection changes glyph presentation, never stored names or SQL policy.
pub fn configure(ctx: &Context, locale: Locale) {
    let cache_id = Id::new("tablecraft_font_locale");
    if ctx.data(|data| data.get_temp::<Locale>(cache_id)) == Some(locale) {
        return;
    }

    let mut fonts = FontDefinitions::default();
    for (name, bytes) in [
        (
            NOTO_EMOJI,
            include_bytes!("../../../../assets/ui/fonts/NotoEmoji-Variable.ttf").as_slice(),
        ),
        (
            SELAWIK,
            include_bytes!("../../../../assets/ui/fonts/selawk.ttf").as_slice(),
        ),
        (
            BOLD_FAMILY,
            include_bytes!("../../../../assets/ui/fonts/selawkb.ttf").as_slice(),
        ),
        (
            NOTO_JAPANESE,
            include_bytes!("../../../../assets/ui/fonts/NotoSansJP-Regular.otf").as_slice(),
        ),
        (
            NOTO_KOREAN,
            include_bytes!("../../../../assets/ui/fonts/NotoSansKR-Regular.otf").as_slice(),
        ),
        (
            NOTO_SIMPLIFIED_CHINESE,
            include_bytes!("../../../../assets/ui/fonts/NotoSansSC-Regular.otf").as_slice(),
        ),
    ] {
        fonts
            .font_data
            .insert(name.to_owned(), FontData::from_static(bytes).into());
    }

    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, SELAWIK.to_owned());
    let preferred = match locale {
        Locale::Ja => [NOTO_JAPANESE, NOTO_KOREAN, NOTO_SIMPLIFIED_CHINESE],
        Locale::Ko => [NOTO_KOREAN, NOTO_JAPANESE, NOTO_SIMPLIFIED_CHINESE],
        _ => [NOTO_SIMPLIFIED_CHINESE, NOTO_JAPANESE, NOTO_KOREAN],
    };
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        let fallback = fonts.families.entry(family).or_default();
        insert_emoji_fallback(fallback);
        fallback.extend(preferred.iter().map(|name| (*name).to_owned()));
    }

    let mut bold_fallback = fonts.families[&FontFamily::Proportional].clone();
    bold_fallback.insert(0, BOLD_FAMILY.to_owned());
    fonts
        .families
        .insert(FontFamily::Name(BOLD_FAMILY.into()), bold_fallback);

    ctx.set_fonts(fonts);
    ctx.data_mut(|data| data.insert_temp(cache_id, locale));
}

/// Select the bold face once egui has applied the installed definitions.
pub(crate) fn bold_family(ui: &Ui) -> FontFamily {
    let family = FontFamily::Name(BOLD_FAMILY.into());
    let installed = ui.fonts(|fonts| fonts.definitions().families.contains_key(&family));
    if installed {
        family
    } else {
        // set_fonts installs definitions on the next pass. A cold host can draw
        // its first widget before that pass; the selection is checked again on
        // each render so the initial proportional face is never cached.
        FontFamily::Proportional
    }
}

fn insert_emoji_fallback(fallback: &mut Vec<String>) {
    // Keep the primary Latin face and old fallbacks, but do not let an older
    // emoji subset shadow the reviewed outlines/sequences. If egui removes its
    // legacy emoji faces, appending is sufficient: there is nothing to shadow.
    let emoji = fallback
        .iter()
        .position(|name| EGUI_EMOJI_FALLBACKS.contains(&name.as_str()))
        .unwrap_or(fallback.len());
    fallback.insert(emoji, NOTO_EMOJI.to_owned());
}

#[cfg(test)]
mod tests;
