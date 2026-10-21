use super::Bindings;
use crate::{EvalError, comparison::Collation};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_sql::select::Expression;

/// Per-evaluation hard ceilings; larger supplied values are clamped to defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvalLimits {
    /// Expression-node nesting, including the root (maximum 32).
    pub max_depth: usize,
    /// Bytes in any individual materialized Text value (maximum 65,536).
    pub max_text_bytes: usize,
}
impl EvalLimits {
    /// Maximum expression-node nesting including the root.
    pub const MAX_DEPTH: usize = 32;
    /// Maximum bytes in one materialized Text value.
    pub const MAX_TEXT_BYTES: usize = 65_536;
}
impl Default for EvalLimits {
    fn default() -> Self {
        Self {
            max_depth: Self::MAX_DEPTH,
            max_text_bytes: Self::MAX_TEXT_BYTES,
        }
    }
}
/// Bounded scalar evaluator. It neither binds a schema nor executes a query.
///
/// One step is charged per visited node; Text allocation bytes are charged
/// cumulatively before allocation, including literals, bindings and intermediates.
/// Host bindings/collations own the limits and side effects of their callbacks.
pub struct Evaluator<'a> {
    pub(super) first_day_of_week: Option<crate::functions::dates::FirstDayOfWeek>,
    pub(super) reference_year: Option<crate::functions::dates::parsing::ReferenceYear>,
    pub(super) date_format: Option<crate::functions::text::cstr::DateTextFormat>,
    pub(super) number_format: Option<crate::functions::conversions::text::NumberFormat>,
    pub(super) character_code_page: Option<crate::functions::text::code_page::CharacterCodePage>,
    pub(super) year_window: crate::functions::dates::TwoDigitYearWindow,
    pub(super) bindings: &'a dyn Bindings,
    pub(super) like_mode: Option<crate::predicates::LikeMode>,
    pub(super) collation: Option<&'a dyn Collation>,
    pub(super) budget: &'a mut ResourceBudget,
    pub(super) limits: EvalLimits,
    pub(super) cancelled: Option<&'a dyn Fn() -> bool>,
}
impl<'a> Evaluator<'a> {
    /// Use explicit host bindings, a consumable operation budget and per-value limits.
    pub fn new(
        bindings: &'a dyn Bindings,
        budget: &'a mut ResourceBudget,
        limits: EvalLimits,
    ) -> Self {
        let ceiling = EvalLimits::default();
        Self {
            bindings,
            budget,
            limits: EvalLimits {
                max_depth: limits.max_depth.min(ceiling.max_depth),
                max_text_bytes: limits.max_text_bytes.min(ceiling.max_text_bytes),
            },
            year_window: crate::functions::dates::TwoDigitYearWindow::ACCESS_DEFAULT,
            collation: None,
            character_code_page: None,
            number_format: None,
            date_format: None,
            reference_year: None,
            first_day_of_week: None,
            like_mode: None,
            cancelled: None,
        }
    }
    /// Supply the host first-day setting used by WeekdayName's system/default option.
    pub fn with_first_day_of_week(mut self, day: crate::functions::dates::FirstDayOfWeek) -> Self {
        self.first_day_of_week = Some(day);
        self
    }
    /// Supply the actual/reference year used by omitted-year date text, without reading a clock.
    pub fn with_reference_year(
        mut self,
        year: crate::functions::dates::parsing::ReferenceYear,
    ) -> Self {
        self.reference_year = Some(year);
        self
    }
    /// Select explicit Gregorian date parsing/output syntax, without OS lookup.
    pub fn with_date_format(
        mut self,
        format: crate::functions::text::cstr::DateTextFormat,
    ) -> Self {
        self.date_format = Some(format);
        self
    }
    /// Select numeric Text input/output syntax for conversion and recognition calls.
    /// No database, OS locale or implicit arithmetic policy is inferred.
    pub fn with_number_format(
        mut self,
        format: crate::functions::conversions::text::NumberFormat,
    ) -> Self {
        self.number_format = Some(format);
        self
    }
    /// Select a validated Gregorian two-digit-year window for DateSerial.
    /// Omission retains the named 1930..2029 profile; no machine setting is read.
    pub fn with_year_window(mut self, window: crate::functions::dates::TwoDigitYearWindow) -> Self {
        self.year_window = window;
        self
    }
    /// Select an explicit numeric character-code policy, independent of database
    /// text storage and comparison. Omission retains ASCII-only byte construction.
    pub fn with_character_code_page(
        mut self,
        page: crate::functions::text::code_page::CharacterCodePage,
    ) -> Self {
        self.character_code_page = Some(page);
        self
    }
    /// Select a host collation for Text comparisons; no default is inferred.
    pub fn with_collation(mut self, collation: &'a dyn Collation) -> Self {
        self.collation = Some(collation);
        self
    }
    /// Select the ANSI wildcard convention for LIKE; no default is inferred.
    pub fn with_like_mode(mut self, mode: crate::predicates::LikeMode) -> Self {
        self.like_mode = Some(mode);
        self
    }
    /// Evaluate LIKE using this evaluator's shared work and text limits.
    pub fn apply_like(
        &mut self,
        value: &Value,
        pattern: &Value,
        negated: bool,
    ) -> Result<Value, EvalError> {
        for operand in [value, pattern] {
            if matches!(operand, Value::Text(text) if text.len() > self.limits.max_text_bytes) {
                return Err(EvalError::TextLimit);
            }
        }
        let result = crate::predicates::like(
            value,
            pattern,
            self.like_mode,
            self.collation,
            self.budget,
            self.cancelled,
        )?;
        Ok(if negated { result.negate() } else { result }.into_value())
    }
    /// Check the host's cancellation flag before every node.
    pub fn with_cancellation(mut self, cancelled: &'a dyn Fn() -> bool) -> Self {
        self.cancelled = Some(cancelled);
        self
    }
    /// Evaluate one supported scalar tree. Both binary operands are evaluated
    /// left-to-right; Boolean truth does not suppress errors in the other operand.
    /// Counters remain consumed across calls, including failed evaluations.
    pub fn evaluate(&mut self, expression: &Expression) -> Result<Value, EvalError> {
        self.node(expression, 1)
    }
    pub(super) fn charge_text(&mut self, length: usize) -> Result<(), EvalError> {
        if length > self.limits.max_text_bytes {
            return Err(EvalError::TextLimit);
        }
        self.budget.charge(Resource::Bytes, length as u64)?;
        Ok(())
    }
    pub(super) fn text(&mut self, text: &str) -> Result<Value, EvalError> {
        self.charge_text(text.len())?;
        let mut result = String::new();
        result
            .try_reserve_exact(text.len())
            .map_err(|_| EvalError::Allocation)?;
        result.push_str(text);
        Ok(Value::Text(result))
    }
}
