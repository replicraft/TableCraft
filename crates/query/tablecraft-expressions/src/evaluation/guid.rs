//! Meter the fixed-size result of an Access GUID byte-array conversion.
use super::Evaluator;
use crate::{EvalError, functions::conversions::guid};
use tablecraft_core::{limits::Resource, values::Value};

impl Evaluator<'_> {
    /// Convert exactly sixteen GUID-structure bytes or a stored Guid to Text.
    /// Result spelling is explicit and independent of locale or CLI formatting.
    pub fn apply_string_from_guid(&mut self, value: &Value) -> Result<Value, EvalError> {
        if self.cancelled.is_some_and(|cancelled| cancelled()) {
            return Err(EvalError::Cancelled);
        }
        self.budget.charge(Resource::Steps, 1)?;
        let bytes = match value {
            Value::Null => return Err(EvalError::InvalidNullConversion),
            Value::Binary(bytes) => bytes
                .as_slice()
                .try_into()
                .map_err(|_| EvalError::TypeMismatch)?,
            Value::Guid(value) => uuid::Uuid::from_bytes(value.to_bytes()).to_bytes_le(),
            _ => return Err(EvalError::TypeMismatch),
        };
        if guid::TEXT_BYTES > self.limits.max_text_bytes {
            return Err(EvalError::TextLimit);
        }
        self.budget
            .charge(Resource::Bytes, guid::TEXT_BYTES as u64)?;
        let mut text = String::new();
        text.try_reserve_exact(guid::TEXT_BYTES)
            .map_err(|_| EvalError::Allocation)?;
        // The fixed formatter emits ASCII punctuation and hexadecimal digits.
        for byte in guid::to_string(bytes) {
            text.push(char::from(byte));
        }
        Ok(Value::Text(text))
    }

    /// Convert evaluated Text to bounded Binary GUID-structure bytes. No implicit
    /// Text coercion is introduced; predicate-only GUID byte admission is separate.
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
    use tablecraft_core::values::Guid;

    #[test]
    fn inverse_conversion_retains_byte_order_and_charges_exact_text_output() {
        let canonical = [
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ];
        let native = Value::Binary(vec![
            0x33, 0x22, 0x11, 0x00, 0x55, 0x44, 0x77, 0x66, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ]);
        for value in [Value::Guid(Guid::from_bytes(canonical)), native] {
            let mut budget = ResourceBudget::new(45, 0, 1);
            let result = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .apply_string_from_guid(&value);
            assert_eq!(
                result,
                Ok(Value::Text(
                    "{guid {00112233-4455-6677-8899-AABBCCDDEEFF}}".into()
                ))
            );
            for (bytes, steps) in [(44, 1), (45, 0)] {
                let mut budget = ResourceBudget::new(bytes, 0, steps);
                assert!(matches!(
                    Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                        .apply_string_from_guid(&value),
                    Err(EvalError::Limit(_))
                ));
            }
            let mut budget = ResourceBudget::new(45, 0, 1);
            assert_eq!(
                Evaluator::new(
                    &NoBindings,
                    &mut budget,
                    EvalLimits {
                        max_text_bytes: 44,
                        ..Default::default()
                    }
                )
                .apply_string_from_guid(&value),
                Err(EvalError::TextLimit)
            );
        }
    }

    #[test]
    fn inverse_refuses_wrong_shapes_and_checks_cancellation_before_conversion() {
        for value in [
            Value::Null,
            Value::Int32(1),
            Value::Text("not bytes".into()),
            Value::Binary(vec![0; 15]),
            Value::Binary(vec![0; 17]),
        ] {
            let mut budget = ResourceBudget::new(100, 0, 100);
            assert_eq!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .apply_string_from_guid(&value),
                Err(if value == Value::Null {
                    EvalError::InvalidNullConversion
                } else {
                    EvalError::TypeMismatch
                })
            );
            assert_eq!(
                Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                    .with_cancellation(&|| true)
                    .apply_string_from_guid(&value),
                Err(EvalError::Cancelled)
            );
        }
    }

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
