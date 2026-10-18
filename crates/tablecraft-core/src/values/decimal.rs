//! Exact field Decimal, retained scale and lossless rescaling.

use std::fmt;

use super::{ValueError, fixed::display_fixed};

/// An exact Access field decimal: signed coefficient and retained scale 0..=28.
///
/// The coefficient has at most 28 decimal digits. A specific column can impose
/// smaller precision/scale limits, which its schema must check. This is not the
/// larger VBA Variant Decimal envelope (96-bit magnitude). No implicit rounding,
/// float conversion, scale normalization or arithmetic promotion is provided.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Decimal {
    coefficient: i128,
    scale: u8,
}

impl Decimal {
    /// Largest absolute coefficient in the 28-digit field envelope.
    pub const MAX_COEFFICIENT: i128 = 10_i128.pow(28) - 1;

    /// Validate exact coefficient and scale without rounding or normalization.
    pub fn new(coefficient: i128, scale: u8) -> Result<Self, ValueError> {
        if scale > 28 {
            return Err(ValueError::DecimalScaleOutOfRange);
        }
        if !(-Self::MAX_COEFFICIENT..=Self::MAX_COEFFICIENT).contains(&coefficient) {
            return Err(ValueError::DecimalOutOfRange);
        }
        Ok(Self { coefficient, scale })
    }

    /// Return the exact signed coefficient.
    pub const fn coefficient(self) -> i128 {
        self.coefficient
    }

    /// Return retained digits after the decimal point.
    pub const fn scale(self) -> u8 {
        self.scale
    }

    /// Change scale only when the result is exact and remains representable.
    pub fn rescale_exact(self, scale: u8) -> Result<Self, ValueError> {
        if scale > 28 {
            return Err(ValueError::DecimalScaleOutOfRange);
        }
        let coefficient = if scale >= self.scale {
            self.coefficient
                .checked_mul(10_i128.pow(u32::from(scale - self.scale)))
                .ok_or(ValueError::DecimalOutOfRange)?
        } else {
            let divisor = 10_i128.pow(u32::from(self.scale - scale));
            if self.coefficient % divisor != 0 {
                return Err(ValueError::DecimalPrecisionLoss);
            }
            self.coefficient / divisor
        };
        Self::new(coefficient, scale)
    }
}

impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        display_fixed(self.coefficient, self.scale, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_validates_access_envelope_without_panicking_on_i128_extremes() {
        let max = Decimal::MAX_COEFFICIENT;
        for scale in 0..=28 {
            for coefficient in [0, 1, -1, max, -max, 9_007_199_254_740_993] {
                let value = Decimal::new(coefficient, scale).unwrap();
                assert_eq!(value.coefficient(), coefficient);
                assert_eq!(value.scale(), scale);
                assert_eq!(
                    Decimal::new(value.coefficient(), value.scale()).unwrap(),
                    value
                );
            }
        }
        for coefficient in [max + 1, -max - 1, i128::MIN, i128::MAX] {
            assert_eq!(
                Decimal::new(coefficient, 0),
                Err(ValueError::DecimalOutOfRange)
            );
        }
        for scale in [29, u8::MAX] {
            assert_eq!(
                Decimal::new(0, scale),
                Err(ValueError::DecimalScaleOutOfRange)
            );
        }
    }

    #[test]
    fn decimal_display_keeps_scale_and_digits_beyond_float_precision() {
        for (coefficient, scale, expected) in [
            (0, 4, "0.0000"),
            (1200, 3, "1.200"),
            (-1200, 3, "-1.200"),
            (9_007_199_254_740_993, 0, "9007199254740993"),
            (1, 28, "0.0000000000000000000000000001"),
            (-1, 28, "-0.0000000000000000000000000001"),
            (Decimal::MAX_COEFFICIENT, 0, "9999999999999999999999999999"),
        ] {
            assert_eq!(
                Decimal::new(coefficient, scale).unwrap().to_string(),
                expected
            );
        }
        // Representation equality deliberately preserves the supplied scale.
        assert_ne!(Decimal::new(10, 1).unwrap(), Decimal::new(100, 2).unwrap());
    }

    #[test]
    fn decimal_rescaling_never_rounds_or_overflows_silently() {
        let value = Decimal::new(-1230, 3).unwrap();
        assert_eq!(
            value.rescale_exact(2).unwrap(),
            Decimal::new(-123, 2).unwrap()
        );
        assert_eq!(
            value.rescale_exact(4).unwrap(),
            Decimal::new(-12300, 4).unwrap()
        );
        assert_eq!(
            value.rescale_exact(1),
            Err(ValueError::DecimalPrecisionLoss)
        );
        assert_eq!(
            value.rescale_exact(29),
            Err(ValueError::DecimalScaleOutOfRange)
        );
        assert_eq!(
            Decimal::new(Decimal::MAX_COEFFICIENT, 0)
                .unwrap()
                .rescale_exact(1),
            Err(ValueError::DecimalOutOfRange)
        );
        assert_eq!(
            Decimal::new(Decimal::MAX_COEFFICIENT, 0)
                .unwrap()
                .rescale_exact(28),
            Err(ValueError::DecimalOutOfRange)
        );
        assert_eq!(
            Decimal::new(0, 0).unwrap().rescale_exact(28).unwrap(),
            Decimal::new(0, 28).unwrap()
        );
    }

    #[test]
    fn deterministic_decimal_rescaling_sweep_round_trips_exact_values() {
        // Fixed, exhaustive small coefficients and every permitted scale: no RNG or locale.
        for coefficient in -100..=100 {
            for scale in 0..=28 {
                let original = Decimal::new(coefficient, scale).unwrap();
                for target in 0..=28 {
                    if let Ok(rescaled) = original.rescale_exact(target) {
                        assert_eq!(rescaled.rescale_exact(scale).unwrap(), original);
                    }
                }
            }
        }
    }
}
