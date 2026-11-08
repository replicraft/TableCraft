//! Bounded syntax presentation does not qualify storage, bind sources or run SQL.
use crate::{Error, display::Presentation};
use std::fmt::{self, Write};
use tablecraft_sql::{ParseLimits, query::Query};

struct Output<'a, 'b> {
    presentation: &'a mut Presentation<'b>,
    text: String,
    error: Option<Error>,
}

impl Write for Output<'_, '_> {
    fn write_str(&mut self, fragment: &str) -> fmt::Result {
        if self.error.is_some() {
            return Err(fmt::Error);
        }
        let result = if self
            .text
            .len()
            .checked_add(fragment.len())
            .is_none_or(|length| length > ParseLimits::MAX_BYTES)
        {
            Err(Error::SqlLimit)
        } else {
            self.presentation.append_text(&mut self.text, fragment)
        };
        if let Err(error) = result {
            self.error = Some(error);
            return Err(fmt::Error);
        }
        Ok(())
    }
}

pub(super) fn query(query: &Query, presentation: &mut Presentation<'_>) -> Result<String, Error> {
    let mut output = Output {
        presentation,
        text: String::new(),
        error: None,
    };
    let formatted = write!(output, "{query}");
    if let Some(error) = output.error {
        return Err(error);
    }
    formatted.map_err(|_| Error::Formatting)?;
    Ok(output.text)
}

#[cfg(test)]
mod tests;
