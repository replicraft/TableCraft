use super::encoding::{encoded, package_bytes};
use super::*;

fn data(name: &str, type_name: &str) -> String {
    DATA.replace("ID", name)
        .replace("<x:schema>", &format!("<x:schema><x:simpleType name=\"{type_name}\"><x:restriction base=\"x:int\"/></x:simpleType>"))
        .replace("type=\"x:int\"", &format!("type=\"{type_name}\""))
}

#[test]
fn valid_non_ascii_xml_names_preserve_stored_names_and_values_in_all_encodings() {
    for (name, type_name, decoded) in [
        ("A·", "À", "A·"),
        ("🦀", "编号", "🦀"),
        ("A_x0020_B", "A\u{203f}", "A B"),
        ("A_x003A_B", "A\u{2040}", "A:B"),
        ("\u{effff}", "\u{2074}", "\u{effff}"),
    ] {
        let text = data(name, type_name);
        for mode in 0..4 {
            let raw = package_bytes(mode, &encoded(&text, mode), CompressionMethod::Deflated);
            let before = raw.clone();
            let package = open(&raw).unwrap();
            let columns = package
                .table_columns("People", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(columns[0].name(), decoded);
            let table = package
                .read_table("People", &mut budget(), &|| false)
                .unwrap();
            assert_eq!(table.rows()[0][0], Value::Int32(1));
            assert_eq!(table.rows()[1][0], Value::Int32(2));
            assert_eq!(raw, before);
        }
    }
}

#[test]
fn malformed_schema_names_and_qnames_fail_before_rows_and_do_not_expand_namespaces() {
    for text in [
        data("A²", "Good"),
        data("Good", "A²"),
        DATA.replace("type=\"x:int\"", "type=\"x:int:extra\""),
        DATA.replace("type=\"x:int\"", "type=\"x:²\""),
        DATA.replace("type=\"x:int\"", "type=\":int\""),
        DATA.replace("type=\"x:int\"", "type=\"missing:int\""),
    ] {
        let end = text.find("</x:schema>").unwrap() + "</x:schema>".len();
        let prefix = format!("{}<broken", &text[..end]);
        for mode in 0..4 {
            let raw = package_bytes(mode, &encoded(&prefix, mode), CompressionMethod::Stored);
            let package = open(&raw).unwrap();
            assert!(
                package
                    .table_columns("People", &mut budget(), &|| false)
                    .is_err()
            );
        }
    }
}

#[test]
fn valid_names_keep_schema_prefix_lazy_and_preserve_cancellation_and_work_limits() {
    let text = data("À", "🦀");
    let end = text.find("</x:schema>").unwrap() + "</x:schema>".len();
    for mode in 0..4 {
        let mut bytes = encoded(&text[..end], mode);
        bytes.extend_from_slice(&[0xff]);
        let raw = package_bytes(mode, &bytes, CompressionMethod::Stored);
        let package = open(&raw).unwrap();
        assert!(
            package
                .table_columns("People", &mut budget(), &|| false)
                .is_ok()
        );
        assert!(
            package
                .read_table("People", &mut budget(), &|| false)
                .is_err()
        );
        assert!(
            package
                .table_columns("People", &mut budget(), &|| true)
                .is_err()
        );
        assert!(
            package
                .table_columns(
                    "People",
                    &mut ResourceBudget::new(64 * 1024 * 1024, 100_000, 1),
                    &|| false
                )
                .is_err()
        );
    }
}

#[test]
fn exact_unicode_prefixes_resolve_by_uri_without_changing_schema_names() {
    for prefix in ["🦀", "A·", "À"] {
        let text = DATA
            .replace("x:", &format!("{prefix}:"))
            .replace("xmlns:x=", &format!("xmlns:{prefix}="));
        let package = fixture(&text, METADATA, OBJECTS, LINKS);
        let table = package
            .read_table("People", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(table.columns()[0].name(), "ID");
        assert_eq!(table.rows()[0][0], Value::Int32(1));
    }
}
