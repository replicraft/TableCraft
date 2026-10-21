//! Meter the fixed-size result of an Access GUID byte-array conversion.
use super::Evaluator;
use crate::{EvalError, functions::conversions::guid};
use tablecraft_core::{limits::Resource, values::Value};

impl Evaluator<'_> {
    /// Convert evaluated Text to bounded Binary GUID-structure bytes. No implicit
    /// Text coercion or semantic GUID/Binary comparison is introduced.
    pub fn apply_guid_from_string(&mut self, value: &Value) -> Result<Value, EvalError> {
        if self.cancelled.is_some_and(|cancelled| cancelled()) {
            return Err(EvalError::Cancelled);
        }
        self.budget.charge(Resource::Steps, 1)?;
        let text = match value {
            Value::Null => return Err(EvalError::InvalidNullConversion),
            Value::Text(text) => text,
            _ => return Err(EvalError::TypeMismatch),
        };
        if text.len() > self.limits.max_text_bytes {
            return Err(EvalError::TextLimit);
        }
        let bytes = guid::from_string(text)?;
        self.budget.charge(Resource::Bytes, bytes.len() as u64)?;
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(bytes.len())
            .map_err(|_| EvalError::Allocation)?;
        owned.extend_from_slice(&bytes);

        Ok(Value::Binary(owned))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluation::{EvalLimits, NoBindings};
    use tablecraft_core::limits::ResourceBudget;

    #[test]
    fn conversion_checks_fixed_output_bytes_steps_cancellation_and_null() {
        let text = Value::Text("6B29FC40-CA47-1067-B31D-00DD010662DA".into());
        for (bytes, steps) in [(15, 100), (100, 0)] {
            let mut budget = ResourceBudget::new(bytes, 0, steps);
            let result = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .apply_guid_from_string(&text);
            assert!(matches!(result, Err(EvalError::Limit(_))));
        }

        let mut budget = ResourceBudget::new(16, 0, 100);
        let value = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .apply_guid_from_string(&text)
            .unwrap();
        assert!(matches!(value, Value::Binary(bytes) if bytes.len() == 16));

        let mut budget = ResourceBudget::new(100, 0, 100);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_cancellation(&|| true)
                .apply_guid_from_string(&text),
            Err(EvalError::Cancelled)
        );
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .apply_guid_from_string(&Value::Null),
            Err(EvalError::InvalidNullConversion)
        );
    }
}
