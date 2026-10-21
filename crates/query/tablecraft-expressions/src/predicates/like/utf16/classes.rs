use super::{States, Work, compare, pattern::ranges};
use crate::{
    EvalError,
    comparison::{Collation, LikeSpanState},
};
use std::cmp::Ordering;

const NOT_ELEMENT: u8 = 0;
const OUTSIDE: u8 = 1;
const INSIDE: u8 = 2;

impl States {
    pub(super) fn class(
        &mut self,
        text: &[u16],
        list: &[u16],
        negated: bool,
        last: bool,
        col: &dyn Collation,
        work: &mut Work<'_>,
    ) -> Result<(), EvalError> {
        for start in 0..text.len() {
            work.tick()?;
            if self.previous[start] == 0 {
                continue;
            }
            let mut forbidden = false;
            for flag in &mut self.candidates {
                work.tick()?;
                *flag = NOT_ELEMENT;
            }
            let first = if last && !negated {
                text.len()
            } else {
                start + 1
            };
            for end in first..=text.len() {
                work.tick()?;
                let candidate = &text[start..end];
                let state = col.like_unit_step(candidate)?;
                work.tick()?;
                match state {
                    LikeSpanState::Impossible => break,
                    LikeSpanState::Possible => continue,
                    LikeSpanState::Match => {}
                }
                let mut found = false;
                for range in ranges(list) {
                    let (low, high) = range?;
                    let lower = compare(&[low], candidate, col, work)?;
                    let upper = compare(candidate, &[high], col, work)?;
                    found |= lower != Ordering::Greater && upper != Ordering::Greater;
                }
                forbidden |= found;
                self.candidates[end] = if found { INSIDE } else { OUTSIDE };
            }
            // An excluded equivalent expansion at this start takes precedence
            // over a shorter nonexcluded prefix. This explicit model avoids
            // letting [!ß]* consume one s and bypass the excluded ss element.
            if negated && forbidden {
                continue;
            }
            for end in start + 1..=text.len() {
                work.tick()?;
                if last && end != text.len() {
                    continue;
                }
                if self.candidates[end] == if negated { OUTSIDE } else { INSIDE } {
                    self.current[end] = 1;
                }
            }
        }
        Ok(())
    }
}
