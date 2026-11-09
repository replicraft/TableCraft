//! Explicit bundled UI languages, separate from database and expression policies.
mod catalogs;

use fluent_bundle::{FluentArgs, FluentResource, concurrent::FluentBundle};
use std::{
    fmt,
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use unic_langid::LanguageIdentifier;

type Bundle = FluentBundle<FluentResource>;

/// Supported presentation languages. No operating-system locale is consulted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Locale {
    #[default]
    EnUs,
    Es,
    De,
    Ru,
    Ja,
    Ko,
    ZhCn,
    PtBr,
}

impl Locale {
    pub const ALL: [Self; 8] = [
        Self::EnUs,
        Self::Es,
        Self::De,
        Self::Ru,
        Self::Ja,
        Self::Ko,
        Self::ZhCn,
        Self::PtBr,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EnUs => "en-US",
            Self::Es => "es",
            Self::De => "de",
            Self::Ru => "ru",
            Self::Ja => "ja",
            Self::Ko => "ko",
            Self::ZhCn => "zh-CN",
            Self::PtBr => "pt-BR",
        }
    }

    pub const fn native_name(self) -> &'static str {
        match self {
            Self::EnUs => "English (US)",
            Self::Es => "Español",
            Self::De => "Deutsch",
            Self::Ru => "Русский",
            Self::Ja => "日本語",
            Self::Ko => "한국어",
            Self::ZhCn => "简体中文",
            Self::PtBr => "Português (Brasil)",
        }
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Locale {
    type Err = LocalizationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        for locale in Self::ALL {
            if value.eq_ignore_ascii_case(locale.as_str()) {
                return Ok(locale);
            }
        }
        // Accept the user's informal Japanese label; emit its actual BCP-47 code.
        if value.eq_ignore_ascii_case("jp") {
            return Ok(Self::Ja);
        }
        Err(LocalizationError::UnsupportedLocale)
    }
}

/// Errors name only bundled resources, never message arguments or database data.
#[derive(Debug, thiserror::Error)]
pub enum LocalizationError {
    #[error("the requested UI language is not supported")]
    UnsupportedLocale,
    #[error("a bundled UI language identifier is invalid")]
    InvalidIdentifier,
    #[error("invalid bundled translation syntax: {locale}/{section}")]
    InvalidResource {
        locale: Locale,
        section: &'static str,
    },
    #[error("conflicting bundled translations: {locale}/{section}")]
    ConflictingResource {
        locale: Locale,
        section: &'static str,
    },
}

/// Payload-free counters shared by clones of one presentation session.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LocalizationDiagnostics {
    pub missing_messages: u64,
    pub formatting_errors: u64,
    pub english_fallbacks: u64,
}

#[derive(Default)]
struct Counters {
    missing_messages: AtomicU64,
    formatting_errors: AtomicU64,
    english_fallbacks: AtomicU64,
}

struct Shared {
    locale: Locale,
    bundle: Bundle,
    english: Option<Bundle>,
    diagnostics: Counters,
}

/// Cloneable concurrent Fluent bundles for explicitly selected UI copy.
/// Bundled catalogs are complete; fallback is a diagnosed exceptional path.
#[derive(Clone)]
pub struct Localizer(Arc<Shared>);

impl Localizer {
    pub fn new(locale: Locale) -> Result<Self, LocalizationError> {
        let bundle = build_bundle(locale)?;
        let english = if locale == Locale::EnUs {
            None
        } else {
            Some(build_bundle(Locale::EnUs)?)
        };
        Ok(Self(Arc::new(Shared {
            locale,
            bundle,
            english,
            diagnostics: Counters::default(),
        })))
    }

    pub fn locale(&self) -> Locale {
        self.0.locale
    }

    pub fn text(&self, id: &str) -> String {
        self.lookup(id, None)
    }

    pub fn format(&self, id: &str, arguments: &FluentArgs<'_>) -> String {
        self.lookup(id, Some(arguments))
    }

    pub fn diagnostics(&self) -> LocalizationDiagnostics {
        let counters = &self.0.diagnostics;
        LocalizationDiagnostics {
            missing_messages: counters.missing_messages.load(Ordering::Relaxed),
            formatting_errors: counters.formatting_errors.load(Ordering::Relaxed),
            english_fallbacks: counters.english_fallbacks.load(Ordering::Relaxed),
        }
    }

    fn lookup(&self, id: &str, arguments: Option<&FluentArgs<'_>>) -> String {
        if let Some(value) = self.resolve(&self.0.bundle, id, arguments) {
            return value;
        }
        if let Some(english) = &self.0.english
            && let Some(value) = self.resolve(english, id, arguments)
        {
            increment(&self.0.diagnostics.english_fallbacks);
            return value;
        }
        // Never display resolver placeholders, private arguments or a raw key.
        self.resolve(&self.0.bundle, "shell-translation-unavailable", None)
            .unwrap_or_else(|| "Translation unavailable".to_owned())
    }

    fn resolve(
        &self,
        bundle: &Bundle,
        id: &str,
        arguments: Option<&FluentArgs<'_>>,
    ) -> Option<String> {
        let Some(pattern) = bundle.get_message(id).and_then(|message| message.value()) else {
            increment(&self.0.diagnostics.missing_messages);
            return None;
        };
        let mut errors = Vec::new();
        let result = bundle.format_pattern(pattern, arguments, &mut errors);
        if errors.is_empty() {
            Some(result.into_owned())
        } else {
            increment(&self.0.diagnostics.formatting_errors);
            None
        }
    }
}

impl Default for Localizer {
    fn default() -> Self {
        Self::new(Locale::EnUs).expect("bundled English UI translations must be valid")
    }
}

fn increment(counter: &AtomicU64) {
    let _ = counter.try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
        Some(value.saturating_add(1))
    });
}

fn build_bundle(locale: Locale) -> Result<Bundle, LocalizationError> {
    let language: LanguageIdentifier = locale
        .as_str()
        .parse()
        .map_err(|_| LocalizationError::InvalidIdentifier)?;
    let mut bundle = Bundle::new_concurrent(vec![language]);
    for (section, source) in catalogs::sources(locale) {
        let resource = FluentResource::try_new(source.to_owned())
            .map_err(|_| LocalizationError::InvalidResource { locale, section })?;
        bundle
            .add_resource(resource)
            .map_err(|_| LocalizationError::ConflictingResource { locale, section })?;
    }
    Ok(bundle)
}

/// Install the selected language for nested presentation functions.
pub(crate) fn install(ctx: &egui::Context, localizer: &Localizer) {
    // NO_CLONE: Share the immutable localization bundle across mutable widget work; Localizer
    // clones its Arc owner, not the catalogs.
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("ui_localizer"), localizer.clone()));
}

pub(crate) fn for_context(ctx: &egui::Context) -> Localizer {
    ctx.data_mut(|data| {
        // NO_CLONE: Share the immutable localization bundle across mutable widget work;
        // Localizer clones its Arc owner, not the catalogs.
        data.get_temp_mut_or_default::<Localizer>(egui::Id::new("ui_localizer"))
            .clone()
    })
}

pub(crate) fn tr(ui: &egui::Ui, id: &str) -> String {
    for_context(ui.ctx()).text(id)
}

/// Format caller-owned or borrowed arguments without retaining their text.
pub(crate) fn message<S: AsRef<str>>(ui: &egui::Ui, id: &str, values: &[(&str, S)]) -> String {
    let mut arguments = FluentArgs::new();
    for (name, value) in values {
        arguments.set(*name, value.as_ref());
    }
    for_context(ui.ctx()).format(id, &arguments)
}

#[cfg(test)]
mod tests;
