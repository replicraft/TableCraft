//! Shared locale-independent fixed-point display mechanics.

use std::fmt;

pub(super) fn display_fixed(
    coefficient: i128,
    scale: u8,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if coefficient < 0 {
        f.write_str("-")?;
    }
    let magnitude = coefficient.unsigned_abs();
    if scale == 0 {
        return write!(f, "{magnitude}");
    }
    let divisor = 10_u128.pow(u32::from(scale));
    write!(
        f,
        "{}.{:0width$}",
        magnitude / divisor,
        magnitude % divisor,
        width = usize::from(scale)
    )
}
