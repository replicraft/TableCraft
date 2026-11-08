//! GetUserDefaultLocaleName queries regional preferences, not UI languages.

use windows_sys::Win32::{
    Globalization::GetUserDefaultLocaleName, System::SystemServices::LOCALE_NAME_MAX_LENGTH,
};

// Windows LOCALE_NAME_MAX_LENGTH includes the terminating UTF-16 NUL.
const LOCALE_NAME_CAPACITY: usize = LOCALE_NAME_MAX_LENGTH as usize;

pub(super) fn locale() -> Option<String> {
    let mut buffer = [0_u16; LOCALE_NAME_CAPACITY];

    // SAFETY: buffer is writable for the supplied element count. The API neither
    // retains the pointer nor writes beyond that count. A successful result
    // includes a NUL, checked below before decoding the initialized prefix.
    let count =
        unsafe { GetUserDefaultLocaleName(buffer.as_mut_ptr(), LOCALE_NAME_CAPACITY as i32) };
    let count = usize::try_from(count).ok()?;
    if count <= 1 || count > buffer.len() || buffer[count - 1] != 0 {
        return None;
    }

    let identifier = String::from_utf16(&buffer[..count - 1]).ok()?;
    super::normalize(&identifier)
}
