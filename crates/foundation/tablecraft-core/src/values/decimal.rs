//! Exact Decimal magnitude, retained scale and lossless rescaling.

use super::{ValueError, fixed::display_fixed};
use std::fmt;

/// An exact Decimal: signed 96-bit magnitude and retained scale 0..=28.
///
/// This covers the VBA Variant Decimal range. Native fields impose the narrower
/// precision/scale constraints owned by schema; constructing a value does not
/// authorize storage in any column. No implicit rounding, float conversion or
/// arithmetic promotion is provided by this carrier.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "DecimalRepr"))]
pub struct Decimal {
    coefficient: i128,
    scale: u8,
}

impl Decimal {
    /// Maximum decimal coefficient digits; the 29-digit top band is bounded by 96 bits.
    pub const MAX_PRECISION: u8 = Self::MAX_COEFFICIENT.ilog10() as u8 + 1;
    /// Maximum retained fractional digits.
    pub const MAX_SCALE: u8 = 28;
    /// Coefficient magnitude width; the sign is stored separately.
    pub const COEFFICIENT_BITS: u32 = 96;
    /// Largest absolute coefficient in the unsigned 96-bit magnitude envelope.
    pub const MAX_COEFFICIENT: i128 = (1_i128 << Self::COEFFICIENT_BITS) - 1;

    /// Validate exact coefficient and scale without rounding or normalization.
    pub fn new(coefficient: i128, scale: u8) -> Result<Self, ValueError> {
        if scale > Self::MAX_SCALE {
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
        if scale > Self::MAX_SCALE {
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

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DecimalRepr {
    coefficient: i128,
    scale: u8,
}

#[cfg(feature = "serde")]
impl TryFrom<DecimalRepr> for Decimal {
    type Error = ValueError;

    fn try_from(repr: DecimalRepr) -> Result<Self, Self::Error> {
        Self::new(repr.coefficient, repr.scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_validates_variant_envelope_without_panicking_on_i128_extremes() {
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
            (Decimal::MAX_COEFFICIENT, 0, "79228162514264337593543950335"),
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

#[cfg(test)]
mod properties {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(crate::test_support::config())]
        #[test]
        fn exact_rescale_restores_the_original(coefficient in -Decimal::MAX_COEFFICIENT..=Decimal::MAX_COEFFICIENT, scale in 0_u8..=28, target in 0_u8..=28) {
            let value = Decimal::new(coefficient, scale).unwrap();
            if let Ok(rescaled) = value.rescale_exact(target) {
                prop_assert_eq!(rescaled.rescale_exact(scale).unwrap(), value);
            }
        }

        #[test]
        fn constructor_acceptance_matches_the_declared_envelope(coefficient in any::<i128>(), scale in any::<u8>()) {
            let expected = scale <= 28 && coefficient.unsigned_abs() <= Decimal::MAX_COEFFICIENT as u128;
            prop_assert_eq!(Decimal::new(coefficient, scale).is_ok(), expected);
        }
    }
}
