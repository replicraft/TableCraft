//! Exact mixed binary key views; scalar comparison and retained cells stay intact.
mod fixed;
pub(super) use fixed::mixed_fixed_pair;
use tablecraft_core::values::{Double, Value};

/// Widen only a mixed binary pair. Existing comparison owns finite validation,
/// family admission, cancellation, text policies and resource checks.
pub(super) fn mixed_float_views(left: &Value, right: &Value) -> Option<(Value, Value)> {
    match (left, right) {
        (Value::Single(left), Value::Double(right)) => Some((
            Value::Double(Double::from_float(f64::from(left.to_float()))),
            Value::Double(*right),
        )),
        (Value::Double(left), Value::Single(right)) => Some((
            Value::Double(*left),
            Value::Double(Double::from_float(f64::from(right.to_float()))),
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
