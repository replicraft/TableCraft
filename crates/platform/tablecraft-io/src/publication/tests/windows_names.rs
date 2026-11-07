use crate::windows_names::valid_component;

#[test]
fn windows_names_reject_streams_devices_separators_and_ambiguous_endings() {
    for name in [
        "",
        ".",
        "..",
        "name.",
        "name ",
        "a:b",
        "a/b",
        "a\\b",
        "a\0b",
        "a\nb",
        "a<b",
        "a>b",
        "a|b",
        "a?b",
        "a*b",
        "a\"b",
        "CON",
        "con.accdb",
        "PRN",
        "AUX.mdb",
        "NUL",
        "COM1",
        "com9.accdb",
        "LPT1",
        "lpt9.mdb",
        "COM¹.accdb",
        "LPT²",
        "COM³",
        "CONIN$",
        "CONOUT$",
        "CON .accdb",
        "COM1 .mdb",
    ] {
        assert!(
            !valid_component(&name.encode_utf16().collect::<Vec<_>>()),
            "{name:?}"
        );
    }
}

#[test]
fn windows_names_accept_unicode_and_nonreserved_similar_names_without_normalizing() {
    for name in [
        "Database1.accdb",
        "contacts.mdb",
        "数据库.accdb",
        "住所록.accdb",
        "résumé.accdb",
        "COM10.accdb",
        "LPT0.accdb",
        "console.accdb",
        "auxiliary.mdb",
        "a b.accdb",
    ] {
        assert!(
            valid_component(&name.encode_utf16().collect::<Vec<_>>()),
            "{name:?}"
        );
    }
}

#[test]
fn windows_names_bound_utf16_length_and_reject_unpaired_surrogates() {
    assert!(valid_component(&vec![b'a' as u16; 255]));
    assert!(!valid_component(&vec![b'a' as u16; 256]));
    assert!(!valid_component(&[0xd800]));
    assert!(!valid_component(&[0xdc00]));
    assert!(valid_component(&[0xd83d, 0xde00]));
}
