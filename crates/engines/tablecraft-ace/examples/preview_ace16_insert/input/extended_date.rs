//! Exact civil input for this engineering protocol, without implicit rounding.
use tablecraft_core::values::ExtendedDateTime;

const INPUT_BYTES: usize = "0001-01-01T00:00:00.0000000".len();
const YEAR_END: usize = 4;
const MONTH_END: usize = 7;
const DATE_END: usize = 10;
const HOUR_END: usize = 13;
const MINUTE_END: usize = 16;
const SECOND_END: usize = 19;
const INVALID: &str = "expected YYYY-MM-DDTHH:MM:SS.fffffff without a timezone";

pub(super) fn parse(bytes: &[u8]) -> Result<ExtendedDateTime, &'static str> {
    if bytes.len() != INPUT_BYTES
        || !bytes
            .iter()
            .enumerate()
            .all(|(position, byte)| match position {
                YEAR_END | MONTH_END => *byte == b'-',
                DATE_END => *byte == b'T',
                HOUR_END | MINUTE_END => *byte == b':',
                SECOND_END => *byte == b'.',
                _ => byte.is_ascii_digit(),
            })
    {
        return Err(INVALID);
    }

    ExtendedDateTime::new(
        digits(&bytes[..YEAR_END]) as u16,
        digits(&bytes[YEAR_END + 1..MONTH_END]) as u8,
        digits(&bytes[MONTH_END + 1..DATE_END]) as u8,
        digits(&bytes[DATE_END + 1..HOUR_END]) as u8,
        digits(&bytes[HOUR_END + 1..MINUTE_END]) as u8,
        digits(&bytes[MINUTE_END + 1..SECOND_END]) as u8,
        digits(&bytes[SECOND_END + 1..]),
    )
    .map_err(|_| INVALID)
}

fn digits(bytes: &[u8]) -> u32 {
    // The fixed ASCII widths above bound every component to at most seven
    // digits, so this accumulation and each narrower component cast fit.
    bytes
        .iter()
        .fold(0, |value, digit| value * 10 + u32::from(digit - b'0'))
}
