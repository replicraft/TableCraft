use super::super::{casing::CaseLocale, code_page::CharacterCodePage};
use super::StringCodePage;
use crate::{EvalError, comparison::LocaleId};

const JAPANESE_LCID: u32 = 1041;
const JAPANESE_CODE_PAGE: u16 = 932;

// Reviewed language/code-page identities from Microsoft's Windows SDK table.
// Match complete LCIDs: alternate sorts and system/default aliases never collapse.
const SUPPORTED_LOCALES: &[(u32, &str, u16)] = &[
    (1025, "ar-SA", 1256),
    (1029, "cs-CZ", 1250),
    (1031, "de-DE", 1252),
    (1032, "el-GR", 1253),
    (1033, "en-US", 1252),
    (1036, "fr-FR", 1252),
    (1037, "he-IL", 1255),
    (1043, "nl-NL", 1252),
    (1049, "ru-RU", 1251),
    (1054, "th-TH", 874),
    (1055, "tr-TR", 1254),
    (1061, "et-EE", 1257),
    (1063, "lt-LT", 1257),
    (1066, "vi-VN", 1258),
];

/// Explicit StrConv LCID profile: ICU casing and an encoding_rs single-byte page.
/// These mapped identities do not establish native Windows NLS equivalence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocaleProfile {
    case_locale: CaseLocale,
    code_page: CharacterCodePage,
}

impl LocaleProfile {
    /// Resolve a complete supported LCID without ambient system discovery.
    pub fn from_lcid(locale: LocaleId) -> Result<Self, EvalError> {
        let (_, language, page) = SUPPORTED_LOCALES
            .iter()
            .find(|(id, _, _)| *id == locale.raw())
            .ok_or(EvalError::UnsupportedExpression)?;
        Ok(Self {
            case_locale: language.parse()?,
            code_page: CharacterCodePage::from_windows_id(*page)?,
        })
    }

    /// Validated explicit casing language.
    pub fn case_locale(&self) -> &CaseLocale {
        &self.case_locale
    }

    /// Validated explicit single-byte character page.
    pub const fn code_page(&self) -> CharacterCodePage {
        self.code_page
    }
}

/// Complete StrConv string language/encoding selection, including Japanese932.
/// The legacy single-byte LocaleProfile API remains independently validated.
/// These explicit ICU/WHATWG identities do not qualify Windows NLS behavior.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StringLocaleProfile {
    case_locale: CaseLocale,
    code_page: StringCodePage,
}

impl StringLocaleProfile {
    /// Resolve exact Japanese1041 or one of the existing single-byte LCIDs.
    /// Alternate-sort/system aliases refuse instead of selecting a default.
    pub fn from_lcid(locale: LocaleId) -> Result<Self, EvalError> {
        if locale.raw() == JAPANESE_LCID {
            return Ok(Self {
                case_locale: "ja-JP".parse()?,
                code_page: StringCodePage::from_windows_id(JAPANESE_CODE_PAGE)?,
            });
        }
        let LocaleProfile {
            case_locale,
            code_page,
        } = LocaleProfile::from_lcid(locale)?;
        Ok(Self {
            case_locale,
            code_page: code_page.into(),
        })
    }

    /// Explicit casing language; no operating-system locale lookup occurs.
    pub fn case_locale(&self) -> &CaseLocale {
        &self.case_locale
    }

    /// Complete single- or multibyte string encoding for this selection.
    pub const fn code_page(&self) -> StringCodePage {
        self.code_page
    }
}
