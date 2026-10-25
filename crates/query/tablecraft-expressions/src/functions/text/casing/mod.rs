//! Explicit ICU Unicode casing; this is not a native Windows NLS qualification.
use super::output::TextOutput;
use crate::EvalError;
use icu_casemap::{
    CaseMapper,
    options::{LeadingAdjustment, TitlecaseOptions},
};
use icu_locale_core::LanguageIdentifier;
use std::{fmt, str::FromStr};
use writeable::Writeable;

// Restrict host policy parsing independently of database text limits.
const MAX_LANGUAGE_BYTES: usize = 128;
// Reviewed ICU contextual rules can inspect preceding and following input.
// Reserve four complete input scans, plus conservative Greek uppercase context.
const CONTEXT_SCAN_PASSES: usize = 4;

// CP932's double-byte space is U+3000. It is an additional word boundary only
// under an explicitly selected Japanese casing language.
const IDEOGRAPHIC_SPACE: char = '\u{3000}';

/// Validated language/script/region/variants, without extensions or OS discovery.
/// `und` explicitly chooses ICU root casing. Other unrecognized languages use
/// ICU's root rules, rather than inventing Windows behavior for those languages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseLocale(LanguageIdentifier);

impl FromStr for CaseLocale {
    type Err = EvalError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.is_empty()
            || text.len() > MAX_LANGUAGE_BYTES
            || !text.is_ascii()
            || text.contains('_')
        {
            return Err(EvalError::InvalidCaseLocale);
        }
        text.parse()
            .map(Self)
            .map_err(|_| EvalError::InvalidCaseLocale)
    }
}

/// Borrowed Text conversion; all output goes through the caller's bounded sink.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Casing<'a> {
    text: &'a str,
    operation: Operation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Operation {
    Upper,
    Lower,
    Proper,
}

impl<'a> Casing<'a> {
    pub(super) fn new(text: &'a str, uppercase: bool) -> Self {
        Self {
            text,
            operation: if uppercase {
                Operation::Upper
            } else {
                Operation::Lower
            },
        }
    }

    pub(super) fn proper(text: &'a str) -> Self {
        Self {
            text,
            operation: Operation::Proper,
        }
    }

    /// Reserve source/context work before ICU, then stream output without an
    /// intermediate String. Host cancellation, byte and allocation errors survive
    /// the fmt::Write adapter unchanged. Conservative context accounting may reject
    /// long Greek uppercase strings before exhausting the byte limit.
    pub fn write(
        self,
        locale: Option<&CaseLocale>,
        output: &mut dyn TextOutput,
    ) -> Result<(), EvalError> {
        output.step()?;
        let locale = locale.ok_or(EvalError::CaseLocaleRequired)?;
        // Sigma and Turkic/Lithuanian scans stop at the next cased/base/above
        // character: across a whole input, their traversals are linear. Greek
        // uppercase additionally scans backwards across each diacritic in a run;
        // conservatively reserve a complete input per non-ASCII scalar there.
        for _ in 0..CONTEXT_SCAN_PASSES {
            for _ in 0..self.text.len() {
                output.step()?;
            }
        }
        if self.operation == Operation::Upper && locale.0.language.as_str() == "el" {
            for character in self.text.chars() {
                output.step()?;
                if !character.is_ascii() {
                    for _ in 0..CONTEXT_SCAN_PASSES {
                        for _ in 0..self.text.len() {
                            output.step()?;
                        }
                    }
                }
            }
        }
        let mut sink = Sink {
            output,
            error: None,
        };
        let mapper = CaseMapper::new();
        let result = match self.operation {
            Operation::Upper => mapper.uppercase(self.text, &locale.0).write_to(&mut sink),
            Operation::Lower => mapper.lowercase(self.text, &locale.0).write_to(&mut sink),
            Operation::Proper => {
                let mut options = TitlecaseOptions::default();
                options.leading_adjustment = Some(LeadingAdjustment::ToCased);
                let japanese = locale.0.language.as_str() == "ja";
                self.text
                    .split_inclusive(|character| {
                        proper_separator(character) || japanese && character == IDEOGRAPHIC_SPACE
                    })
                    .try_for_each(|segment| {
                        mapper
                            .titlecase_segment_with_only_case_data(segment, &locale.0, options)
                            .write_to(&mut sink)
                    })
            }
        };
        if let Some(error) = sink.error {
            return Err(error);
        }
        result.map_err(|_| EvalError::UnrepresentableText)?;
        sink.output.step()
    }
}

fn proper_separator(character: char) -> bool {
    matches!(
        character,
        '\0' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' '
    )
}

struct Sink<'a> {
    output: &'a mut dyn TextOutput,
    error: Option<EvalError>,
}
impl fmt::Write for Sink<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if self.error.is_some() {
            return Err(fmt::Error);
        }
        match self.output.step().and_then(|()| self.output.append(text)) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.error = Some(error);
                Err(fmt::Error)
            }
        }
    }
}

#[cfg(test)]
mod tests;
