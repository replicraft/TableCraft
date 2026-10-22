//! Interpret only unescaped Access date fields; quoted user text stays inert.
use super::super::lexer::{self, Kind, Tokens};
use crate::EvalError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Meridiem {
    Upper,
    Lower,
    UpperInitial,
    LowerInitial,
    Regional,
}

pub(super) enum Field<'a> {
    Literal(&'a str),
    Punctuation(char),
    Day(usize),
    Month(usize),
    Year(usize),
    Hour(usize),
    Minute(usize),
    Second(usize),
    Weekday,
    Week,
    Quarter,
    General,
    LongTime,
    DateSeparator,
    TimeSeparator,
    Meridiem(Meridiem),
}

pub(super) struct Scanner<'a> {
    tokens: Tokens<'a>,
    previous_hour: bool,
}

impl<'a> Scanner<'a> {
    pub fn new(mask: &'a str) -> Self {
        Self {
            tokens: Tokens::new(mask),
            previous_hour: false,
        }
    }

    pub fn next(
        &mut self,
        step: &mut impl FnMut() -> Result<(), EvalError>,
    ) -> Result<Option<Field<'a>>, EvalError> {
        let Some(token) = self.tokens.next(step)? else {
            return Ok(None);
        };
        let symbol = match token.kind {
            Kind::Literal(text) => return Ok(Some(Field::Literal(text))),
            Kind::Symbol(symbol) => symbol,
        };
        if matches!(symbol, 'a' | 'A') {
            for (suffix, uppercase, lowercase) in [
                ("M/PM", Meridiem::Upper, Meridiem::Lower),
                ("/P", Meridiem::UpperInitial, Meridiem::LowerInitial),
                ("MPM", Meridiem::Regional, Meridiem::Regional),
            ] {
                if self
                    .tokens
                    .remaining()
                    .get(..suffix.len())
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(suffix))
                {
                    self.tokens.consume_ascii(suffix.len(), step)?;
                    self.previous_hour = false;
                    return Ok(Some(Field::Meridiem(if symbol == 'A' {
                        uppercase
                    } else {
                        lowercase
                    })));
                }
            }
            return Err(EvalError::UnsupportedExpression);
        }
        let lower = symbol.to_ascii_lowercase();
        if matches!(
            lower,
            'd' | 'm' | 'y' | 'h' | 'n' | 's' | 'w' | 'q' | 'c' | 't'
        ) {
            let mut count = 1;
            while self
                .tokens
                .remaining()
                .chars()
                .next()
                .is_some_and(|next| next.eq_ignore_ascii_case(&symbol))
            {
                self.tokens.consume_ascii(symbol.len_utf8(), step)?;
                count += 1;
            }
            let field = match (lower, count) {
                ('d', 1..=6) => Field::Day(count),
                ('m', 1 | 2) if self.previous_hour => Field::Minute(count),
                ('m', 1..=4) => Field::Month(count),
                ('y', 1 | 2 | 4) => Field::Year(count),
                ('h', 1 | 2) => Field::Hour(count),
                ('n', 1 | 2) => Field::Minute(count),
                ('s', 1 | 2) => Field::Second(count),
                ('w', 1) => Field::Weekday,
                ('w', 2) => Field::Week,
                ('q', 1) => Field::Quarter,
                ('c', 1) => Field::General,
                ('t', 5) => Field::LongTime,
                _ => return Err(EvalError::UnsupportedExpression),
            };
            self.previous_hour = lower == 'h';
            return Ok(Some(field));
        }
        Ok(Some(match symbol {
            '/' => Field::DateSeparator,
            ':' => Field::TimeSeparator,
            ',' | '.' => Field::Punctuation(symbol),
            symbol if lexer::literal(symbol) => Field::Punctuation(symbol),
            _ => return Err(EvalError::UnsupportedExpression),
        }))
    }
}

pub(super) fn contains_date(
    mask: &str,
    step: &mut impl FnMut() -> Result<(), EvalError>,
) -> Result<bool, EvalError> {
    let mut tokens = Tokens::new(mask);
    while let Some(token) = tokens.next(step)? {
        if let Kind::Symbol(symbol) = token.kind
            && matches!(
                symbol.to_ascii_lowercase(),
                'd' | 'm' | 'y' | 'h' | 'n' | 's' | 'w' | 'q' | 'c' | 't' | 'a'
            )
        {
            return Ok(true);
        }
    }
    Ok(false)
}
