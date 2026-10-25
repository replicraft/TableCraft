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
    /// Opt-in ICU case-insensitive/accent-sensitive host text profile.
    /// Use an explicit supported language-region name, such as en-US or ja-JP.
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

fn parse_locale(value: &str) -> Result<IcuTextLocale, String> {
    IcuTextLocale::from_name(value).ok_or_else(|| {
        let supported = IcuTextLocale::ALL
            .iter()
            .map(|locale| locale.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        format!("expected explicit ICU locale: {supported}")
    })
}

fn parse_lcid(value: &str) -> Result<LocaleId, &'static str> {
    let raw = value
        .parse::<u32>()
        .map_err(|_| "expected positive numeric LCID")?;
    LocaleId::new(raw).map_err(|_| "unsupported LCID structure")
}
