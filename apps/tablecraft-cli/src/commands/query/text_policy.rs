//! Explicit text-mode provider selection, independent of database policies.
use clap::Args as ClapArgs;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::{
    EvalError,
    comparison::{
        Collation, LocaleId,
        icu_text::{IcuTextCollation, IcuTextLocale},
    },
};

#[derive(Clone, Copy, Default, ClapArgs)]
#[group(id = "icu_text_policy")]
pub(crate) struct Args {
    /// Opt-in ICU case-insensitive/accent-sensitive text profile: en-US, de-DE, sv-SE or tr-TR. Not native NLS qualification.
    #[arg(long, value_name = "LOCALE", value_parser = parse_locale)]
    pub(crate) text_locale: Option<IcuTextLocale>,
    /// Explicitly register the selected ICU profile for this exact InStr LCID; no numeric aliases are inferred.
    #[arg(long, value_name = "LCID", requires = "text_locale", value_parser = parse_lcid)]
    text_lcid: Option<LocaleId>,
}

impl Args {
    pub(super) fn build<'a>(
        self,
        base: Option<&'a dyn Collation>,
        budget: ResourceBudget,
        cancelled: &'a dyn Fn() -> bool,
    ) -> Result<Option<IcuTextCollation<'a>>, EvalError> {
        let Some(locale) = self.text_locale else {
            return Ok(None);
        };
        let mut provider = IcuTextCollation::new(locale, budget, cancelled)?;
        if let Some(base) = base {
            provider = provider.with_database(base);
        }
        if let Some(lcid) = self.text_lcid {
            provider = provider.with_registered_locale(lcid);
        }
        Ok(Some(provider))
    }
}

fn parse_locale(value: &str) -> Result<IcuTextLocale, &'static str> {
    match value.to_ascii_lowercase().as_str() {
        "en-us" => Ok(IcuTextLocale::EnUs),
        "de-de" => Ok(IcuTextLocale::DeDe),
        "sv-se" => Ok(IcuTextLocale::SvSe),
        "tr-tr" => Ok(IcuTextLocale::TrTr),
        _ => Err("expected explicit ICU locale en-US, de-DE, sv-SE or tr-TR"),
    }
}

fn parse_lcid(value: &str) -> Result<LocaleId, &'static str> {
    let raw = value
        .parse::<u32>()
        .map_err(|_| "expected positive numeric LCID")?;
    LocaleId::new(raw).map_err(|_| "unsupported LCID structure")
}
