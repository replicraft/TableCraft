use super::*;
use crate::template::{tree, xml};

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 1_000_000)
}

#[test]
fn exact_start_boundaries_are_independent_of_unicode_character_categories() {
    let accepted = [
        0x41, 0x5a, 0x5f, 0x61, 0x7a, 0xc0, 0xd6, 0xd8, 0xf6, 0xf8, 0x2ff, 0x370, 0x37d, 0x37f,
        0x1fff, 0x200c, 0x200d, 0x2070, 0x218f, 0x2c00, 0x2fef, 0x3001, 0xd7ff, 0xf900, 0xfdcf,
        0xfdf0, 0xfffd, 0x10000, 0x1f980, 0x2074, 0xeffff,
    ];
    for scalar in accepted {
        let name = char::from_u32(scalar).unwrap().to_string();
        assert!(ncname(&name), "{scalar:x}");
        assert!(ncname(&format!("A{name}")), "{scalar:x}");
    }

    for scalar in [
        0x40, 0x5b, 0x5e, 0x60, 0x7b, 0xbf, 0xd7, 0xf7, 0x36f, 0x37e, 0x2000, 0x200b, 0x200e,
        0x206f, 0x2190, 0x2bff, 0x2ff0, 0x3000, 0xe000, 0xf8ff, 0xfdd0, 0xfdef, 0xfffe, 0xffff,
        0xf0000, 0x10ffff,
    ] {
        assert!(
            !ncname(&char::from_u32(scalar).unwrap().to_string()),
            "{scalar:x}"
        );
    }

    for scalar in [0xb2, 0xb3, 0xb9, 0x37e, 0xf0000] {
        let character = char::from_u32(scalar).unwrap();
        assert!(!ncname(&format!("A{character}")), "{scalar:x}");
    }
}

#[test]
fn continuation_and_qname_boundaries_do_not_normalize_or_fold_names() {
    for character in [
        '0', '9', '-', '.', '\u{b7}', '\u{300}', '\u{36f}', '\u{203f}', '\u{2040}',
    ] {
        assert!(!ncname(&character.to_string()));
        assert!(ncname(&format!("A{character}")));
    }
    for name in ["", ":", "A:B", " A", "A ", "A\n", "A\0"] {
        assert!(!ncname(name));
    }
    for name in ["A", "A:B", "🦀:A\u{300}", "_x0020_", "编号"] {
        assert!(qname(name, &mut budget()).is_ok());
    }
    for name in ["", ":A", "A:", "A:B:C", "A²", "A:²", "A B"] {
        assert!(matches!(
            qname(name, &mut budget()),
            Err(TemplateError::Malformed)
        ));
    }
    assert!(qname("A", &mut ResourceBudget::new(100, 100, 0)).is_err());
}

#[test]
fn every_xml_entrypoint_validates_event_names_and_namespace_declaration_names() {
    for text in [
        "<A² xmlns='urn:x'/>",
        "<A xmlns='urn:x' bad²='x'/>",
        "<A xmlns='urn:x' xmlns:²='urn:y'/>",
        "<A xmlns='urn:x' xmlns:p='urn:y'><p:B:C/></A>",
        "<A xmlns='urn:x' xmlns:p='urn:y' p:B:C='x'/>",
    ] {
        assert!(matches!(
            tree::parse(text.as_bytes(), &mut budget(), &|| false),
            Err(TemplateError::Malformed)
        ));
        assert!(matches!(
            xml::records(text.as_bytes(), "urn:x", "A", &mut budget(), &|| false),
            Err(TemplateError::Malformed)
        ));
    }
    for text in [
        "<A xmlns='urn:x'><🦀 A·='x'/></A>",
        "<p:A xmlns:p='urn:x'><p:À/></p:A>",
    ] {
        assert!(tree::parse(text.as_bytes(), &mut budget(), &|| false).is_ok());
        assert!(xml::records(text.as_bytes(), "urn:x", "A", &mut budget(), &|| false).is_ok());
    }
    assert!(matches!(
        tree::parse(b"<p:A/>", &mut budget(), &|| false),
        Err(TemplateError::Malformed)
    ));
}
