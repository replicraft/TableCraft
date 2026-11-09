//! One bounded wildcard frontier retaining the best consumed source origin.
use super::{Origins, States, pattern};
use crate::comparison::StringInput;
use crate::{
    EvalError,
    comparison::{Collation, LikeProfile, LikeSpanState, StringSearch},
    predicates::like::{
        HIGH_SURROGATES, LOW_SURROGATES, LikeCase, LikeMatch, LikeMode, Work, reachable,
    },
};
use std::{cmp::Ordering, ops::Range};

struct Policy<'a> {
    source: &'a dyn Collation,
    case: LikeCase,
}

impl Collation for Policy<'_> {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        self.source.compare(left, right)
    }

    fn like_profile(&self) -> LikeProfile {
        match self.case {
            LikeCase::Database => self.source.like_profile(),
            LikeCase::Sensitive => self.source.sensitive_like_profile(),
        }
    }

    fn compare_like_units(&self, left: &[u16], right: &[u16]) -> Result<Ordering, EvalError> {
        match self.case {
            LikeCase::Database => self.source.compare_like_units(left, right),
            LikeCase::Sensitive => self.source.compare_sensitive_like_units(left, right),
        }
    }

    fn like_literal_step(
        &self,
        candidate: &[u16],
        literal: &[u16],
    ) -> Result<LikeSpanState, EvalError> {
        match self.case {
            LikeCase::Database => self.source.like_literal_step(candidate, literal),
            LikeCase::Sensitive => self.source.sensitive_like_literal_step(candidate, literal),
        }
    }

    fn like_unit_step(&self, candidate: &[u16]) -> Result<LikeSpanState, EvalError> {
        match self.case {
            LikeCase::Database => self.source.like_unit_step(candidate),
            LikeCase::Sensitive => self.source.sensitive_like_unit_step(candidate),
        }
    }
}

fn boundary(units: &[u16], offset: usize) -> bool {
    offset <= units.len()
        && !(offset > 0
            && offset < units.len()
            && HIGH_SURROGATES.contains(&units[offset - 1])
            && LOW_SURROGATES.contains(&units[offset]))
}

/// One strictly validated wildcard pattern reusable across source values.
/// Compilation and every subsequent search share caller-owned cumulative quotas.
pub struct PreparedLikeSearch {
    pattern: pattern::Pattern,
    matching: LikeMatch,
    case: LikeCase,
}

impl PreparedLikeSearch {
    /// Compile the explicit ANSI grammar once. Invalid syntax refuses before a
    /// host scans source records; comparison-dependent ranges validate on use.
    pub fn new(
        expression: StringInput<'_>,
        mode: LikeMode,
        matching: LikeMatch,
        case: LikeCase,
        budget: &mut tablecraft_core::limits::ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<Self, EvalError> {
        let mut work = Work { budget, cancelled };
        work.tick()?;
        let pattern = pattern::compile(expression, mode, &mut work)?;
        for token in &pattern.tokens {
            work.tick()?;
            if matches!(token, pattern::Token::Invalid) {
                return Err(EvalError::InvalidPattern);
            }
        }
        Ok(Self {
            pattern,
            matching,
            case,
        })
    }

    /// Return a source UTF-16 span under the selected provider and cursor bounds.
    pub fn find(
        &self,
        text: StringInput<'_>,
        search: StringSearch,
        collation: &dyn Collation,
        budget: &mut tablecraft_core::limits::ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<Option<Range<usize>>, EvalError> {
        self.search(text, search, collation, &mut Work { budget, cancelled })
    }

    fn search(
        &self,
        text: StringInput<'_>,
        search: StringSearch,
        collation: &dyn Collation,
        work: &mut Work<'_>,
    ) -> Result<Option<Range<usize>>, EvalError> {
        work.tick()?;
        let matching = self.matching;
        let pattern = &self.pattern;
        let policy = Policy {
            source: collation,
            case: self.case,
        };
        if policy.like_profile() != LikeProfile::Utf16Weights {
            return Err(EvalError::UnsupportedExpression);
        }
        let units = text.units(work.budget, work.cancelled)?;
        let (lower, upper, reverse) = match search {
            StringSearch::ForwardFrom(start) => (start, units.len(), false),
            StringSearch::ReverseThrough(end) => (0, end, true),
        };
        if !boundary(&units, lower) || !boundary(&units, upper) {
            return Err(EvalError::InvalidTextMatch);
        }
        let width = units.len().checked_add(1).ok_or(EvalError::Allocation)?;
        let mut states = States {
            previous: work.buffer(width)?,
            current: work.buffer(width)?,
            candidates: work.buffer(width)?,
            origins: Some(Origins {
                previous: work.buffer(width)?,
                current: work.buffer(width)?,
                reverse,
            }),
        };
        states.previous.resize(width, 0);
        states.current.resize(width, 0);
        states.candidates.resize(width, 0);
        let origins = states
            .origins
            .as_mut()
            .ok_or(EvalError::UnsupportedExpression)?;
        origins.previous.resize(width, usize::MAX);
        origins.current.resize(width, usize::MAX);
        for start in lower..=upper {
            work.tick()?;
            if boundary(&units, start) && (matching == LikeMatch::Any || start == 0) {
                states.previous[start] = 1;
                origins.previous[start] = start;
            }
        }
        for token in &pattern.tokens {
            work.tick()?;
            if !reachable(&states.previous, work)? {
                return Ok(None);
            }
            for index in 0..width {
                work.tick()?;
                states.current[index] = 0;
                states
                    .origins
                    .as_mut()
                    .ok_or(EvalError::UnsupportedExpression)?
                    .current[index] = usize::MAX;
            }
            if !states.advance(&units[..upper], pattern, token, false, false, &policy, work)? {
                return Ok(None);
            }
            std::mem::swap(&mut states.previous, &mut states.current);
            let origins = states
                .origins
                .as_mut()
                .ok_or(EvalError::UnsupportedExpression)?;
            std::mem::swap(&mut origins.previous, &mut origins.current);
        }
        let origins = states
            .origins
            .as_ref()
            .ok_or(EvalError::UnsupportedExpression)?;
        let mut best: Option<Range<usize>> = None;
        for end in lower..=upper {
            work.tick()?;
            if states.previous[end] == 0
                || !boundary(&units, end)
                || (matching == LikeMatch::Whole && end != units.len())
            {
                continue;
            }
            let start = origins.previous[end];
            if best.as_ref().is_none_or(|old| {
                (if reverse {
                    start > old.start
                } else {
                    start < old.start
                }) || start == old.start
                    && ((old.is_empty() && end > start)
                        || (!old.is_empty() && end > start && end < old.end))
            }) {
                best = Some(start..end);
            }
        }
        Ok(best)
    }
}
