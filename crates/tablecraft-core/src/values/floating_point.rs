//! Single and Double carriers preserving IEEE representations.

macro_rules! float_value {
    ($name:ident, $bits:ty, $float:ty, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "serde", serde(transparent))]
        pub struct $name($bits);

        impl $name {
            /// Preserve all IEEE bits, including signed zero and NaN payloads.
            pub const fn from_bits(bits: $bits) -> Self {
                Self(bits)
            }

            /// Preserve the bit representation of a Rust float.
            pub const fn from_float(value: $float) -> Self {
                Self(value.to_bits())
            }

            /// Return the original bit representation.
            pub const fn to_bits(self) -> $bits {
                self.0
            }

            /// Read as a float; this does not define Access comparison semantics.
            pub const fn to_float(self) -> $float {
                <$float>::from_bits(self.0)
            }
        }
    };
}

float_value!(
    Single,
    u32,
    f32,
    "A 32-bit float carrier with bitwise equality, not SQL equality."
);
float_value!(
    Double,
    u64,
    f64,
    "A 64-bit float carrier with bitwise equality, not SQL equality."
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_carriers_preserve_ieee_special_values_and_payloads() {
        for bits in [
            0,
            0x80000000,
            0x7f800000,
            0xff800000,
            0x7fc00001,
            0x7fc00002,
            u32::MAX,
        ] {
            assert_eq!(
                Single::from_float(Single::from_bits(bits).to_float()).to_bits(),
                bits
            );
        }
        for bits in [
            0,
            0x8000000000000000,
            0x7ff0000000000000,
            0xfff0000000000000,
            0x7ff8000000000001,
            0x7ff8000000000002,
            u64::MAX,
        ] {
            assert_eq!(
                Double::from_float(Double::from_bits(bits).to_float()).to_bits(),
                bits
            );
        }
        assert_ne!(Single::from_float(0.0), Single::from_float(-0.0));
        assert_eq!(
            Double::from_bits(0x7ff8000000000001),
            Double::from_bits(0x7ff8000000000001)
        );
    }
}
