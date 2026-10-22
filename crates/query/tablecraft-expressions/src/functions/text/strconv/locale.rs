use super::super::{casing::CaseLocale, code_page::CharacterCodePage};
use crate::{EvalError, comparison::LocaleId};

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
