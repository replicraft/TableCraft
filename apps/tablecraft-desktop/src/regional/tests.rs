use super::{from_environment, normalize};
use std::ffi::OsString;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;

fn resolve(values: &[(&str, &str)]) -> Option<String> {
    from_environment(|key| {
        values
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| OsString::from(value))
    })
}

#[test]
fn regional_categories_take_precedence_over_ui_language() {
    let values = [
        ("LANGUAGE", "en_US"),
        ("LC_MESSAGES", "en_US.UTF-8"),
        ("LANG", "fr_FR.UTF-8"),
        ("LC_MONETARY", "ja_JP.UTF-8"),
        ("LC_TIME", "de_DE.UTF-8"),
    ];
    assert_eq!(resolve(&values).as_deref(), Some("de-DE"));

    let mut overridden = values.to_vec();
    overridden.push(("LC_ALL", "ko_KR.UTF-8"));
    assert_eq!(resolve(&overridden).as_deref(), Some("ko-KR"));
}

#[test]
fn empty_categories_skip_but_selected_unavailable_locale_does_not() {
    assert_eq!(
        resolve(&[("LC_ALL", ""), ("LC_TIME", ""), ("LANG", "pt_BR.UTF-8")]).as_deref(),
        Some("pt-BR")
    );
    for selected in [
        "C",
        "C.UTF-8",
        "POSIX",
        "sr_RS@latin",
        "de_DE@calendar=gregorian",
    ] {
        assert_eq!(resolve(&[("LC_TIME", selected), ("LANG", "de_DE")]), None);
    }

    assert_eq!(resolve(&[("LANGUAGE", "en_US")]), None);
    assert_eq!(
        resolve(&[("LC_TIME", "invalid locale"), ("LANG", "de_DE")]).as_deref(),
        Some("invalid locale")
    );
}

#[test]
fn apple_and_posix_identifiers_are_normalized_without_inventing_overrides() {
    assert_eq!(normalize("de_DE@calendar=gregorian"), None);
    assert_eq!(normalize("zh_Hans_CN.UTF-8").as_deref(), Some("zh-Hans-CN"));
    assert_eq!(normalize("en-US").as_deref(), Some("en-US"));
}

#[cfg(unix)]
#[test]
fn non_unicode_selected_category_does_not_fall_through() {
    assert_eq!(
        from_environment(|key| match key {
            "LC_TIME" => Some(OsString::from_vec(vec![0xff])),
            "LANG" => Some(OsString::from("de_DE")),
            _ => None,
        }),
        None
    );
}
