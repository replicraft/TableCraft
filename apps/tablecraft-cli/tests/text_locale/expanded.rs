use super::*;

#[test]
fn every_supported_host_locale_reaches_comparison_and_canonical_search() {
    for locale in [
        "en-US", "de-DE", "sv-SE", "tr-TR", "fr-FR", "es-ES", "it-IT", "nl-NL", "pl-PL", "cs-CZ",
        "ja-JP", "zh-CN", "zh-TW", "ru-RU", "el-GR", "ar-SA", "he-IL", "ko-KR", "pt-BR",
    ] {
        let output = query(
            "SELECT StrComp('é','é',1) AS C,InStr(1,'😀Éx','é',1) AS N",
            &["--text-locale", locale],
        );
        assert!(
            output.status.success(),
            "{locale}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"C\tN\n0\t3\n", "{locale}");
    }
}

#[test]
fn language_tailorings_and_explicit_registration_remain_separate() {
    for (locale, sql, expected) in [
        (
            "cs-CZ",
            "SELECT StrComp('h','ch',1) AS C",
            b"C\n-1\n".as_slice(),
        ),
        (
            "es-ES",
            "SELECT StrComp('chico','cielo',1) AS C",
            b"C\n-1\n".as_slice(),
        ),
        (
            "ja-JP",
            "SELECT StrComp('あ','ア',1) AS C",
            b"C\n0\n".as_slice(),
        ),
        (
            "zh-CN",
            "SELECT StrComp('阿','八',1) AS C",
            b"C\n-1\n".as_slice(),
        ),
        (
            "zh-TW",
            "SELECT StrComp('阿','八',1) AS C",
            b"C\n1\n".as_slice(),
        ),
        (
            "el-GR",
            "SELECT StrComp('σ','ς',1) AS C",
            b"C\n0\n".as_slice(),
        ),
    ] {
        let output = query(sql, &["--text-locale", locale]);
        assert!(output.status.success(), "{locale}");
        assert_eq!(output.stdout, expected, "{locale}");
    }

    let sql = "SELECT InStr(1,'xア','あ',1041) AS N";
    let output = query(sql, &["--text-locale", "JA-jp", "--text-lcid", "1041"]);
    assert!(output.status.success());
    assert_eq!(output.stdout, b"N\n2\n");
    for options in [
        vec!["--text-locale", "ja-JP"],
        vec!["--text-locale", "ja-JP", "--text-lcid", "1033"],
        vec!["--text-locale", "ja-JP", "--text-lcid", "66577"],
    ] {
        let output = query(sql, &options);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}
