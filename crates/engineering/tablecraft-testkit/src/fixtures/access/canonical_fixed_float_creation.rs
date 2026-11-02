//! Independent IEEE bit-pattern continuation of the frozen fixed SOURCE owner.
use super::canonical_fixed_creation::{self, Source};
use tablecraft_core::values::{Double, Single, Value};

/// A separate raw-float SOURCE; the five original fixed recipes stay unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    /// Minimum and maximum positive subnormal payloads.
    PositiveSubnormal,
    /// Minimum and maximum negative subnormal payloads.
    NegativeSubnormal,
    /// Positive and negative infinity.
    Infinity,
    /// Positive and negative quiet NaNs with asymmetric retained payloads.
    QuietNaN,
    /// Positive and negative signalling NaNs with asymmetric retained payloads.
    SignallingNaN,
}

/// Every independent continuation in its immutable qualification order.
pub const CASES: [Case; 5] = [
    Case::PositiveSubnormal,
    Case::NegativeSubnormal,
    Case::Infinity,
    Case::QuietNaN,
    Case::SignallingNaN,
];

impl Case {
    /// Stable source identity without a machine path or payload diagnostic.
    pub const fn id(self) -> &'static str {
        match self {
            Self::PositiveSubnormal => "positive-subnormal",
            Self::NegativeSubnormal => "negative-subnormal",
            Self::Infinity => "infinity",
            Self::QuietNaN => "quiet-nan",
            Self::SignallingNaN => "signalling-nan",
        }
    }
}

/// Literal (Single bits, Double bits) for the two rows; no float arithmetic.
pub const fn float_bits(case: Case) -> [(u32, u64); 2] {
    match case {
        Case::PositiveSubnormal => [
            (0x0000_0001, 0x0000_0000_0000_0001),
            (0x007f_ffff, 0x000f_ffff_ffff_ffff),
        ],
        Case::NegativeSubnormal => [
            (0x8000_0001, 0x8000_0000_0000_0001),
            (0x807f_ffff, 0x800f_ffff_ffff_ffff),
        ],
        Case::Infinity => [
            (0x7f80_0000, 0x7ff0_0000_0000_0000),
            (0xff80_0000, 0xfff0_0000_0000_0000),
        ],
        Case::QuietNaN => [
            (0x7fc1_2345, 0x7ff8_1234_5678_9abc),
            (0xffe5_4321, 0xfff8_fedc_ba98_7654),
        ],
        Case::SignallingNaN => [
            (0x7f81_2345, 0x7ff0_1234_5678_9abc),
            (0xff85_4321, 0xfff0_fedc_ba98_7654),
        ],
    }
}

/// Overlay only two explicit scalar payloads on the unchanged signed-zero SOURCE.
/// No native encoder, production creator or SDK result influences these bytes.
pub fn source(case: Case) -> Source {
    let mut source = canonical_fixed_creation::source(canonical_fixed_creation::Case::SignedZero);
    for (row, (single, double)) in float_bits(case).into_iter().enumerate() {
        // Independent literal packing: DATA page 8, row size 63, unaligned fixed
        // offsets 23/27 after its two-byte count prefix. Directory lookup and
        // decoder APIs are deliberately not used to select the expected spans.
        let start = 8 * 4096 + 4096 - (row + 1) * 63;
        source.bytes[start + 25..start + 29].copy_from_slice(&single.to_le_bytes());
        source.bytes[start + 29..start + 37].copy_from_slice(&double.to_le_bytes());
        source.rows[row][6] = Value::Single(Single::from_bits(single));
        source.rows[row][7] = Value::Double(Double::from_bits(double));
    }
    source
}
