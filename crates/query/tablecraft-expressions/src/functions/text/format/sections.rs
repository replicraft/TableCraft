//! Bounded borrowed mask sections; quote/escape rules are shared with rendering.
use super::lexer::{Kind, Tokens};
use crate::EvalError;

pub(super) const MAX_FORMAT_UNITS: usize = 257;
const MAX_SECTIONS: usize = 4;
const NEGATIVE_SECTION: usize = 1;
const ZERO_SECTION: usize = 2;
const NULL_SECTION: usize = 3;

pub(super) struct Sections<'a> {
    parts: [&'a str; MAX_SECTIONS],
    count: usize,
}

impl<'a> Sections<'a> {
    pub fn parse(
        mask: &'a str,
        step: &mut impl FnMut() -> Result<(), EvalError>,
    ) -> Result<Self, EvalError> {
        let mut end = 0;
        let mut units = 0;
        for character in mask.chars() {
            step()?;
            let next = units + character.len_utf16();
            if next > MAX_FORMAT_UNITS {
                if units < MAX_FORMAT_UNITS {
                    return Err(EvalError::UnrepresentableText);
                }
                break;
            }
            units = next;
            end += character.len_utf8();
        }
        let mask = &mask[..end];
        let mut sections = Self {
            parts: [""; MAX_SECTIONS],
            count: 1,
        };
        let mut tokens = Tokens::new(mask);
        let mut start = 0;
        while let Some(token) = tokens.next(step)? {
            if token.kind == Kind::Symbol(';') {
                if sections.count == MAX_SECTIONS {
                    return Err(EvalError::Domain);
                }
                sections.parts[sections.count - 1] = &mask[start..token.start];
                sections.count += 1;
                start = token.end;
            }
        }
        sections.parts[sections.count - 1] = &mask[start..];
        Ok(sections)
    }

    pub fn first(&self) -> &'a str {
        self.parts[0]
    }

    pub fn is_single(&self) -> bool {
        self.count == 1
    }

    pub fn numeric(&self, negative: bool, zero: bool) -> (&'a str, bool) {
        let section = if negative && self.count > NEGATIVE_SECTION {
            NEGATIVE_SECTION
        } else if zero && self.count > ZERO_SECTION {
            ZERO_SECTION
        } else {
            0
        };
        if self.parts[section].is_empty() {
            (self.parts[0], negative)
        } else {
            (self.parts[section], negative && section == 0)
        }
    }

    pub fn null(&self) -> &'a str {
        if self.count == MAX_SECTIONS {
            self.parts[NULL_SECTION]
        } else {
            ""
        }
    }
}
