const MAX_COMPONENT_UNITS: usize = 255;
const SPACE: u16 = b' ' as u16;
const DOT: u16 = b'.' as u16;
const FIRST_CONTROL_EXCLUDED: u16 = 32;
const UNSAFE_CHARACTERS: &[u16] = &[34, 42, 47, 58, 60, 62, 63, 92, 124];
const RESERVED_NUMBER_SUFFIXES: &[u16] = &[0x00b9, 0x00b2, 0x00b3];

fn uppercase(unit: u16) -> u16 {
    if (b'a' as u16..=b'z' as u16).contains(&unit) {
        unit - (b'a' - b'A') as u16
    } else {
        unit
    }
}

/// Qualified ordinary Win32 component: valid UTF-16, no streams or device names.
pub(super) fn valid_component(name: &[u16]) -> bool {
    if name.is_empty() || name.len() > MAX_COMPONENT_UNITS {
        return false;
    }
    if char::decode_utf16(name.iter().copied()).any(|unit| unit.is_err()) {
        return false;
    }
    if matches!(name.last(), Some(&SPACE | &DOT)) {
        return false;
    }
    if name
        .iter()
        .any(|unit| *unit < FIRST_CONTROL_EXCLUDED || UNSAFE_CHARACTERS.contains(unit))
    {
        return false;
    }

    let stem = name.split(|unit| *unit == DOT).next().unwrap_or_default();
    let end = stem
        .iter()
        .rposition(|unit| *unit != SPACE)
        .map_or(0, |index| index + 1);
    let stem = &stem[..end];
    let reserved = ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"];
    if reserved
        .iter()
        .any(|word| stem.iter().copied().map(uppercase).eq(word.encode_utf16()))
    {
        return false;
    }
    if stem.len() == 4 {
        let prefix = [uppercase(stem[0]), uppercase(stem[1]), uppercase(stem[2])];
        let numbered = prefix == [b'C' as u16, b'O' as u16, b'M' as u16]
            || prefix == [b'L' as u16, b'P' as u16, b'T' as u16];
        if numbered
            && ((b'1' as u16..=b'9' as u16).contains(&stem[3])
                || RESERVED_NUMBER_SUFFIXES.contains(&stem[3]))
        {
            return false;
        }
    }
    true
}
