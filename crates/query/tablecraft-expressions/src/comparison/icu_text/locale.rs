//! Closed host locale selection, independent of native database orders and LCIDs.
use icu_collator::CollatorPreferences;
use icu_locale_core::locale;

/// Closed, explicitly selected ICU linguistic locales. No OS discovery occurs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IcuTextLocale {
    /// English (United States).
    EnUs,
    /// German (Germany), standard collation.
    DeDe,
    /// Swedish (Sweden).
    SvSe,
    /// Turkish (Turkey).
    TrTr,
    /// French (France).
    FrFr,
    /// Spanish (Spain), standard modern collation.
    EsEs,
    /// Italian (Italy).
    ItIt,
    /// Dutch (Netherlands).
    NlNl,
    /// Polish (Poland).
    PlPl,
    /// Czech (Czechia).
    CsCz,
    /// Japanese (Japan).
    JaJp,
    /// Chinese (China), default locale collation.
    ZhCn,
    /// Chinese (Taiwan), default locale collation.
    ZhTw,
    /// Russian (Russia).
    RuRu,
    /// Greek (Greece).
    ElGr,
    /// Arabic (Saudi Arabia).
    ArSa,
    /// Hebrew (Israel).
    HeIl,
    /// Korean (South Korea).
    KoKr,
    /// Portuguese (Brazil).
    PtBr,
}

impl IcuTextLocale {
    /// All supported explicit host profiles, without numeric LCID aliases.
    pub const ALL: &'static [Self] = &[
        Self::EnUs,
        Self::DeDe,
        Self::SvSe,
        Self::TrTr,
        Self::FrFr,
        Self::EsEs,
        Self::ItIt,
        Self::NlNl,
        Self::PlPl,
        Self::CsCz,
        Self::JaJp,
        Self::ZhCn,
        Self::ZhTw,
        Self::RuRu,
        Self::ElGr,
        Self::ArSa,
        Self::HeIl,
        Self::KoKr,
        Self::PtBr,
    ];

    /// Match an exact language-region name, ignoring ASCII letter case only.
    /// Unknown names, extensions, whitespace and alternate separators are refused.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|locale| locale.as_str().eq_ignore_ascii_case(name))
    }

    /// Exact host locale name, separate from any LCID registration.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EnUs => "en-US",
            Self::DeDe => "de-DE",
            Self::SvSe => "sv-SE",
            Self::TrTr => "tr-TR",
            Self::FrFr => "fr-FR",
            Self::EsEs => "es-ES",
            Self::ItIt => "it-IT",
            Self::NlNl => "nl-NL",
            Self::PlPl => "pl-PL",
            Self::CsCz => "cs-CZ",
            Self::JaJp => "ja-JP",
            Self::ZhCn => "zh-CN",
            Self::ZhTw => "zh-TW",
            Self::RuRu => "ru-RU",
            Self::ElGr => "el-GR",
            Self::ArSa => "ar-SA",
            Self::HeIl => "he-IL",
            Self::KoKr => "ko-KR",
            Self::PtBr => "pt-BR",
        }
    }

    pub(super) fn preferences(self) -> CollatorPreferences {
        match self {
            Self::EnUs => locale!("en-US").into(),
            Self::DeDe => locale!("de-DE").into(),
            Self::SvSe => locale!("sv-SE").into(),
            Self::TrTr => locale!("tr-TR").into(),
            Self::FrFr => locale!("fr-FR").into(),
            Self::EsEs => locale!("es-ES").into(),
            Self::ItIt => locale!("it-IT").into(),
            Self::NlNl => locale!("nl-NL").into(),
            Self::PlPl => locale!("pl-PL").into(),
            Self::CsCz => locale!("cs-CZ").into(),
            Self::JaJp => locale!("ja-JP").into(),
            Self::ZhCn => locale!("zh-CN").into(),
            Self::ZhTw => locale!("zh-TW").into(),
            Self::RuRu => locale!("ru-RU").into(),
            Self::ElGr => locale!("el-GR").into(),
            Self::ArSa => locale!("ar-SA").into(),
            Self::HeIl => locale!("he-IL").into(),
            Self::KoKr => locale!("ko-KR").into(),
            Self::PtBr => locale!("pt-BR").into(),
        }
    }
}
