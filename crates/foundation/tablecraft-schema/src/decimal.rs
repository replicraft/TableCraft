//! Declared field Decimal constraints, independent of numeric expression ranges.
use crate::SchemaError;
use tablecraft_core::values::Decimal;

/// A field's precision and exact retained scale, with no implicit rescaling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecimalConstraints {
    precision: u8,
    scale: u8,
}
impl DecimalConstraints {
    /// Maximum declared Access field precision, independent of expression carriers.
    pub const MAX_PRECISION: u8 = 28;
    /// Validate Access field precision 1..=28 and scale 0..=precision.
    pub fn new(precision: u8, scale: u8) -> Result<Self, SchemaError> {
        if !(1..=Self::MAX_PRECISION).contains(&precision) || scale > precision {
            return Err(SchemaError::TypeConstraint);
        }
        Ok(Self { precision, scale })
    }
    /// Maximum coefficient digits in the declaration.
    pub const fn precision(self) -> u8 {
        self.precision
    }
    /// Required retained fractional digits.
    pub const fn scale(self) -> u8 {
        self.scale
    }
    /// Check the exact declared representation without rounding or rescaling.
    pub fn validate(self, value: Decimal) -> Result<(), SchemaError> {
        if value.scale() != self.scale
            || value.coefficient().unsigned_abs() >= 10_u128.pow(u32::from(self.precision))
        {
            return Err(SchemaError::Value);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_field_declaration_checks_both_signed_bounds_and_retained_scale() {
        for precision in 1..=28 {
            for scale in 0..=precision {
                let field = DecimalConstraints::new(precision, scale).unwrap();
                assert_eq!((field.precision(), field.scale()), (precision, scale));
                let limit = 10_i128.pow(u32::from(precision));
                for coefficient in [0, 1, -1, limit - 1, -limit + 1] {
                    assert_eq!(
                        field.validate(Decimal::new(coefficient, scale).unwrap()),
                        Ok(())
                    );
                }
                {
                    for coefficient in [limit, -limit] {
                        assert_eq!(
                            field.validate(Decimal::new(coefficient, scale).unwrap()),
                            Err(SchemaError::Value)
                        );
                    }
                }
                if scale < 28 {
                    assert_eq!(
                        field.validate(Decimal::new(0, scale + 1).unwrap()),
                        Err(SchemaError::Value)
                    );
                }
            }
        }
        for (precision, scale) in [(0, 0), (29, 0), (255, 0), (1, 2), (28, 29)] {
            assert_eq!(
                DecimalConstraints::new(precision, scale),
                Err(SchemaError::TypeConstraint)
            );
        }
    }
}
