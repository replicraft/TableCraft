//! Host regional identifiers, independent of the selected UI language.
//!
//! The shared formatter currently has one locale. On Unix, LC_TIME selects it;
//! separate LC_MONETARY settings and custom OS patterns are not imported.

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;

pub(super) fn locale() -> Option<String> {
    #[cfg(windows)]
    {
        windows::locale()
    }

    #[cfg(target_os = "macos")]
    {
        let identifier = objc2_foundation::NSLocale::currentLocale().localeIdentifier();
        normalize(&identifier.to_string())
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        from_environment(|key| std::env::var_os(key))
    }

    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

#[cfg(any(test, all(unix, not(target_os = "macos"))))]
fn from_environment(mut lookup: impl FnMut(&str) -> Option<std::ffi::OsString>) -> Option<String> {
    for key in ["LC_ALL", "LC_TIME", "LANG"] {
        let Some(value) = lookup(key).filter(|value| !value.is_empty()) else {
            continue;
        };

        // A selected but invalid identifier must not fall through to a different
        // category or the UI language. The formatter reports unavailable locales.
        return value.to_str().and_then(normalize);
    }

    None
}

fn normalize(identifier: &str) -> Option<String> {
    // Script/calendar modifiers carry semantics this Gregorian display profile
    // does not import. Refuse rather than silently reinterpret the locale.
    if identifier.contains('@') {
        return None;
    }

    let identifier = identifier.split('.').next()?;
    if identifier.is_empty() || matches!(identifier, "C" | "POSIX") {
        return None;
    }

    Some(identifier.replace('_', "-"))
}

#[cfg(test)]
mod tests;
