use super::*;
use std::io::{Cursor, Write};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

const TYPES: &str = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="text/xml"/><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/></Types>"#;
const ROOT: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="r1" Type="http://schemas.microsoft.com/office/access/2005/04/template/start" Target="template/main.xml"/></Relationships>"#;
const METADATA: &str = "<Template/>";
const RELS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="r1" Type="http://schemas.microsoft.com/office/access/2005/04/template/object" Target="../tables/contacts.xml"/></Relationships>"#;

fn entries() -> Vec<(&'static str, &'static str)> {
    vec![
        ("[Content_Types].xml", TYPES),
        ("_rels/.rels", ROOT),
        ("template/main.xml", METADATA),
        ("template/_rels/main.xml.rels", RELS),
        ("tables/contacts.xml", "<Contacts/>"),
    ]
}

pub(super) fn package(entries: &[(&str, &str)], method: CompressionMethod) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, body) in entries {
        writer
            .start_file(
                *name,
                SimpleFileOptions::default().compression_method(method),
            )
            .unwrap();
        writer.write_all(body.as_bytes()).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

pub(super) fn open(bytes: &[u8]) -> Result<TemplatePackage, TemplateError> {
    TemplatePackage::open(
        &mut Cursor::new(bytes),
        &mut ResourceBudget::new(64 * 1024 * 1024, 0, 1_000_000),
        &|| false,
    )
}

#[test]
fn stored_and_deflated_packages_expose_inert_parts_and_resolved_relationships() {
    for method in [CompressionMethod::Stored, CompressionMethod::Deflated] {
        let bytes = package(&entries(), method);
        let template = open(&bytes).unwrap();
        assert_eq!(template.parts().len(), 2);
        assert_eq!(template.metadata().name(), "template/main.xml");
        assert_eq!(template.metadata().bytes(), METADATA.as_bytes());
        let relationship = &template.metadata().relationships()[0];
        assert_eq!(relationship.id(), "r1");
        assert_eq!(relationship.target(), "tables/contacts.xml");
        assert_eq!(
            template.part(relationship.target()).unwrap().bytes(),
            b"<Contacts/>"
        );
        assert_eq!(template.metadata().content_type(), "text/xml");
        assert_eq!(template.relationships().len(), 1);
    }
}

#[test]
fn rejects_duplicate_unsafe_names_and_corrupt_local_headers_or_crc() {
    let mut duplicate = entries();
    duplicate.push(("TABLES/contacts.xml", "<Other/>"));
    assert!(matches!(
        open(&package(&duplicate, CompressionMethod::Stored)),
        Err(TemplateError::Malformed)
    ));
    for name in [
        "../escape.xml",
        "/root.xml",
        "a\\b.xml",
        "a%2fb.xml",
        "a//b.xml",
    ] {
        let mut unsafe_entries = entries();
        unsafe_entries.push((name, "<Bad/>"));
        assert!(matches!(
            open(&package(&unsafe_entries, CompressionMethod::Stored)),
            Err(TemplateError::Unsupported)
        ));
    }
    let bytes = package(&entries(), CompressionMethod::Stored);
    for at in [6, 8, 14, 18, 22, 30] {
        let mut bad = bytes.clone();
        bad[at] ^= 1;
        assert!(open(&bad).is_err(), "local offset {at}");
    }
    let mut bad = bytes.clone();
    let at = bad
        .windows(METADATA.len())
        .position(|window| window == METADATA.as_bytes())
        .unwrap();
    bad[at] ^= 1;
    assert!(matches!(open(&bad), Err(TemplateError::Malformed)));
    for size in [0, 21, bytes.len() - 1] {
        assert!(open(&bytes[..size]).is_err());
    }
}

#[test]
fn opc_relationship_and_content_type_failures_never_publish_a_package() {
    for root in [
        ROOT.replace("template/main.xml", "https://example.invalid/main.xml"),
        ROOT.replace("template/main.xml", "../../outside.xml"),
        ROOT.replace("template/main.xml", "missing.xml"),
        ROOT.replace("Target=", "TargetMode=\"External\" Target="),
        ROOT.replace("template/start", "template/wrong"),
        ROOT.replace(
            "</Relationships>",
            &format!(
                "{} </Relationships>",
                &ROOT[ROOT.find("<Relationship Id").unwrap()
                    ..ROOT.find("</Relationships>").unwrap()]
            ),
        ),
        ROOT.replace("package/2006/relationships", "package/2099/relationships"),
        format!("<!DOCTYPE x [<!ENTITY e SYSTEM 'file:///secret'>]>{ROOT}"),
        ROOT.replace("<Relationship Id", "<Relationship Extra=\"x\" Id"),
    ] {
        let mut parts = entries();
        parts[1].1 = &root;
        assert!(
            open(&package(&parts, CompressionMethod::Deflated)).is_err(),
            "{root}"
        );
    }
    for types in [
        TYPES.replace("text/xml", "application/octet-stream"),
        TYPES.replace("Extension=\"xml\"", "Extension=\"missing\""),
        TYPES.replace(
            "</Types>",
            "<Default Extension=\"xml\" ContentType=\"text/xml\"/></Types>",
        ),
    ] {
        let mut parts = entries();
        parts[0].1 = &types;
        assert!(open(&package(&parts, CompressionMethod::Stored)).is_err());
    }
}

#[test]
fn ceilings_and_every_observed_cancellation_checkpoint_are_enforced() {
    use std::cell::Cell;
    let mut parts = entries();
    parts.push(("empty/", ""));
    let bytes = package(&parts, CompressionMethod::Deflated);
    let calls = Cell::new(0usize);
    let invoke = |stop| {
        calls.set(0);
        TemplatePackage::open(
            &mut Cursor::new(&bytes),
            &mut ResourceBudget::new(64 * 1024 * 1024, 0, 1_000_000),
            &|| {
                calls.set(calls.get() + 1);
                calls.get() == stop
            },
        )
    };
    assert!(invoke(usize::MAX).is_ok());
    let total = calls.get();
    for stop in 1..=total {
        assert!(matches!(
            invoke(stop),
            Err(TemplateError::Read(ReadError::Cancelled))
        ));
    }
    for budget in [
        ResourceBudget::new(4, 0, 100000),
        ResourceBudget::new(1000000, 0, 1),
    ] {
        assert!(matches!(
            TemplatePackage::open(&mut Cursor::new(&bytes), &mut { budget }, &|| false),
            Err(TemplateError::Read(ReadError::Limit(_)))
        ));
    }
    let mut bytes = bytes;
    let central = bytes
        .windows(4)
        .position(|window| window == b"PK\x01\x02")
        .unwrap();
    bytes[central + 24..central + 28].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(open(&bytes), Err(TemplateError::Unsupported)));
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(64))]
    #[test]
    fn mutated_package_geometry_and_xml_are_bounded(
        changes in proptest::collection::vec((proptest::prelude::any::<usize>(), proptest::prelude::any::<u8>()), 1..12),
    ) {
        let mut bytes = package(&entries(), CompressionMethod::Stored);
        for (position, byte) in changes {
            let index = position % bytes.len();
            bytes[index] = byte;
        }
        let _ = open(&bytes);
    }
}

#[test]
fn namespace_prefixes_content_overrides_and_absolute_internal_targets_work() {
    let types = TYPES.replace(
        "</Types>",
        "<Override PartName=\"/tables/contacts.xml\" ContentType=\"application/xml\"/></Types>",
    );
    let root = ROOT
        .replace("<Relationships xmlns=", "<p:Relationships xmlns:p=")
        .replace("<Relationship ", "<p:Relationship ")
        .replace("</Relationships>", "</p:Relationships>")
        .replace("Target=\"template", "Target=\"/template");
    let mut parts = entries();
    parts[0].1 = &types;
    parts[1].1 = &root;
    let template = open(&package(&parts, CompressionMethod::Deflated)).unwrap();
    assert_eq!(
        template.part("tables/contacts.xml").unwrap().content_type(),
        "application/xml"
    );
}

#[test]
fn content_selection_rewinds_native_input_and_rejects_non_template_zip() {
    let mut source = Cursor::new(b"native database bytes");
    source.set_position(8);
    let mut budget = ResourceBudget::new(64 * 1024 * 1024, 0, 1000000);
    assert!(
        TemplatePackage::open_if_template(&mut source, &mut budget, &|| false)
            .unwrap()
            .is_none()
    );
    assert_eq!(source.position(), 0);
    let bytes = package(&entries(), CompressionMethod::Deflated);
    assert!(
        TemplatePackage::open_if_template(&mut Cursor::new(bytes), &mut budget, &|| false)
            .unwrap()
            .is_some()
    );
    assert!(
        TemplatePackage::open_if_template(
            &mut Cursor::new(b"PK\x03\x04malformed"),
            &mut budget,
            &|| false
        )
        .is_err()
    );
}

#[test]
fn opc_ascii_identity_is_shared_by_overrides_relationships_and_lookup() {
    let types = TYPES.replace(
        "</Types>",
        "<Override PartName=\"/TABLES/CONTACTS.XML\" ContentType=\"application/xml\"/></Types>",
    );
    let root = ROOT.replace("template/main.xml", "TEMPLATE/MAIN.XML");
    let relationships = RELS.replace("../tables/contacts.xml", "../TABLES/CONTACTS.XML");
    let mut parts = entries();
    parts[0].1 = &types;
    parts[1] = ("_RELS/.RELS", &root);
    parts[2].0 = "Template/Main.Xml";
    parts[3] = ("TEMPLATE/_RELS/MAIN.XML.RELS", &relationships);
    parts[4].0 = "Tables/Contacts.Xml";
    for method in [CompressionMethod::Stored, CompressionMethod::Deflated] {
        let template = open(&package(&parts, method)).unwrap();
        assert_eq!(template.parts().len(), 2);
        assert_eq!(template.metadata().name(), "Template/Main.Xml");
        let target = template.metadata().relationships()[0].target();
        assert_eq!(target, "Tables/Contacts.Xml");
        let table = template.part("tAbLeS/cOnTaCtS.xMl").unwrap();
        assert_eq!(table.content_type(), "application/xml");
        assert_eq!(table.bytes(), b"<Contacts/>");
        assert!(template.part(&"x".repeat(1025)).is_none());
    }
}

#[test]
fn equivalent_overrides_are_ambiguous_and_relationship_types_remain_exact() {
    let types = TYPES.replace(
        "</Types>",
        "<Override PartName=\"/tables/contacts.xml\" ContentType=\"application/xml\"/><Override PartName=\"/TABLES/CONTACTS.XML\" ContentType=\"text/xml\"/></Types>",
    );
    let mut parts = entries();
    parts[0].1 = &types;
    assert!(matches!(
        open(&package(&parts, CompressionMethod::Stored)),
        Err(TemplateError::Malformed)
    ));

    let root = ROOT.replace("template/start", "template/START");
    parts = entries();
    parts[1].1 = &root;
    assert!(matches!(
        open(&package(&parts, CompressionMethod::Stored)),
        Err(TemplateError::Malformed)
    ));
}

#[test]
fn opc_identity_does_not_apply_unicode_case_folding() {
    let mut parts = entries();
    parts.push(("tables/É.xml", "<Upper/>"));
    parts.push(("tables/é.xml", "<Lower/>"));
    let template = open(&package(&parts, CompressionMethod::Stored)).unwrap();
    assert_eq!(template.part("TABLES/É.XML").unwrap().bytes(), b"<Upper/>");
    assert_eq!(template.part("TABLES/é.XML").unwrap().bytes(), b"<Lower/>");
}

#[test]
fn escaped_utf8_and_opaque_ascii_names_resolve_to_stored_parts() {
    for (stored, reference, logical) in [
        (
            "tables/caf%C3%A9.xml",
            "../tables/caf%C3%A9.xml",
            "tables/café.xml",
        ),
        (
            "tables/%F0%9F%A6%80.xml",
            "../tables/🦀.xml",
            "tables/🦀.xml",
        ),
        (
            "tables/a%20b.xml",
            "../tables/a%20b.xml",
            "tables/a%20b.xml",
        ),
        (
            "tables/%252e.xml",
            "../tables/%252e.xml",
            "tables/%252e.xml",
        ),
        ("tables/a:b.xml", "../tables/a:b.xml", "tables/a:b.xml"),
    ] {
        let relationships = RELS.replace("../tables/contacts.xml", reference);
        let types = TYPES.replace(
            "</Types>",
            &format!("<Override PartName=\"/{stored}\" ContentType=\"application/xml\"/></Types>"),
        );
        let mut parts = entries();
        parts[0].1 = &types;
        parts[3].1 = &relationships;
        parts[4].0 = stored;
        for method in [CompressionMethod::Stored, CompressionMethod::Deflated] {
            let template = open(&package(&parts, method)).unwrap();
            let target = template.metadata().relationships()[0].target();
            assert_eq!(target, logical);
            let part = template.part(target).unwrap();
            assert_eq!(part.name(), logical);
            assert_eq!(part.content_type(), "application/xml");
            assert_eq!(part.bytes(), b"<Contacts/>");
        }
    }
}

#[test]
fn logical_aliases_prefix_parts_and_malformed_escapes_are_refused() {
    for extra in [
        vec![
            ("tables/café.xml", "<One/>"),
            ("tables/caf%C3%A9.xml", "<Two/>"),
        ],
        vec![("TABLES", "<Prefix/>")],
        vec![("tables/a%41.xml", "<Bad/>")],
        vec![("tables/a%FF.xml", "<Bad/>")],
        vec![("tables/a./b.xml", "<Bad/>")],
    ] {
        let mut parts = entries();
        parts.extend(extra);
        assert!(open(&package(&parts, CompressionMethod::Stored)).is_err());
    }
    let mut parts = entries();
    parts.push(("tables/a+b.xml", "<Plus/>"));
    parts.push(("tables/a%2Bb.xml", "<Escaped/>"));
    let template = open(&package(&parts, CompressionMethod::Stored)).unwrap();
    assert_eq!(template.part("tables/a+b.xml").unwrap().bytes(), b"<Plus/>");
    assert_eq!(
        template.part("tables/a%2bb.xml").unwrap().bytes(),
        b"<Escaped/>"
    );
}

#[test]
fn relationship_control_paths_use_only_the_immediate_parent_segment() {
    let mut parts = entries();
    parts.push(("extra/_rels/ordinary.xml", "<Ordinary/>"));
    parts.push(("extra/_rels/folder/ordinary.xml", "<Nested/>"));
    let template = open(&package(&parts, CompressionMethod::Stored)).unwrap();
    assert_eq!(
        template.part("extra/_rels/ordinary.xml").unwrap().bytes(),
        b"<Ordinary/>"
    );
    assert_eq!(
        template
            .part("extra/_rels/folder/ordinary.xml")
            .unwrap()
            .bytes(),
        b"<Nested/>"
    );
}

#[test]
fn empty_directory_records_do_not_become_parts_or_require_content_types() {
    for method in [CompressionMethod::Stored, CompressionMethod::Deflated] {
        let mut parts = entries();
        parts.extend([
            ("template/", ""),
            ("tables/", ""),
            ("_rels/", ""),
            ("empty/", ""),
            ("empty/caf%C3%A9/", ""),
        ]);
        let bytes = package(&parts, method);
        let template = open(&bytes).unwrap();
        assert_eq!(template.parts().len(), 2);
        assert!(template.part("empty").is_none());
        assert!(template.part("empty/").is_none());
        assert_eq!(
            template.metadata().relationships()[0].target(),
            "tables/contacts.xml"
        );
    }
}

#[test]
fn directory_records_still_require_safe_names_empty_content_and_valid_crc() {
    for (directory, body) in [
        ("empty/", "unexpected data"),
        ("../escape/", ""),
        ("/root/", ""),
        ("a%2fb/", ""),
        ("a//b/", ""),
        ("a./", ""),
    ] {
        let mut parts = entries();
        parts.push((directory, body));
        assert!(open(&package(&parts, CompressionMethod::Stored)).is_err());
    }
    let mut parts = entries();
    parts.push(("empty/", ""));
    let mut bytes = package(&parts, CompressionMethod::Stored);
    // Mutate both CRC declarations: consistency passes, EOF validation must fail.
    let local = bytes
        .windows(6)
        .position(|window| window == b"empty/")
        .unwrap()
        - 30;
    let central_name = bytes
        .windows(6)
        .rposition(|window| window == b"empty/")
        .unwrap();
    let central = central_name - 46;
    bytes[local + 14..local + 18].copy_from_slice(&1u32.to_le_bytes());
    bytes[central + 16..central + 20].copy_from_slice(&1u32.to_le_bytes());
    assert!(matches!(open(&bytes), Err(TemplateError::Malformed)));
}

#[test]
fn duplicate_directory_records_and_empty_symlinks_are_not_ignored() {
    let mut parts = entries();
    parts.extend([("empty/", ""), ("EMPTY/", "")]);
    let bytes = package(&parts, CompressionMethod::Stored);
    assert!(open(&bytes).is_ok());
    let mut duplicate = bytes;
    for position in 0..duplicate.len() - b"EMPTY/".len() {
        if &duplicate[position..position + b"EMPTY/".len()] == b"EMPTY/" {
            duplicate[position..position + b"empty/".len()].copy_from_slice(b"empty/");
        }
    }
    assert!(matches!(open(&duplicate), Err(TemplateError::Malformed)));

    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, body) in entries() {
        writer
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(body.as_bytes()).unwrap();
    }
    writer
        .add_symlink("symlink/", "", SimpleFileOptions::default())
        .unwrap();
    let bytes = writer.finish().unwrap().into_inner();
    assert!(matches!(open(&bytes), Err(TemplateError::Unsupported)));
}
