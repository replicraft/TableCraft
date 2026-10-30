//! Read complete native Extra sources before launching a program-inspection CLI.
use super::{extra_fixture, local_table, payload_fixture};
use std::io::Cursor;
use tablecraft_ace::{NativeImage, PayloadPresence, TableExtraCoverage};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::{CatalogScope, ObjectKind};

// Include the actual independently authored fixture owner once in this target.
#[path = "../../../../crates/engines/tablecraft-ace/src/preservation/table_extra/program_fixtures.rs"]
#[allow(dead_code)]
mod program_fixtures;

use csv::ReaderBuilder;
use program_fixtures::Outcome;

#[test]
fn qualify_macro_program_extra_sources_before_consumer() {
    for code in [5, 6] {
        for source in program_fixtures::sources() {
            let value = extra_fixture::text(source.xml);
            let raw = extra_fixture::bag(&[
                extra_fixture::names(&["StoredRule"]),
                extra_fixture::group(3, &[extra_fixture::property(0, 1, 12, &value)]),
            ]);
            let mut bytes = local_table(payload_fixture::inline(&raw), false);
            assert_eq!(bytes[20], 6);
            bytes[20] = code;
            let original = bytes.clone();
            let mut budget = ResourceBudget::new(64 * 1024 * 1024, 100_000, 20_000_000);
            let image = NativeImage::capture(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                &mut budget,
                &|| false,
            )
            .unwrap();
            let object = image
                .catalog()
                .catalog()
                .lookup("Items", Some(ObjectKind::Table), None)
                .unwrap()
                .unwrap();
            let extra = image
                .inspect_table_extra(object.id(), &mut budget, &|| false)
                .unwrap();
            assert_eq!(extra.presence(), PayloadPresence::Present);
            assert_eq!(extra.coverage(), TableExtraCoverage::WideMr2);
            assert_eq!(extra.native_id(), 10);
            assert_eq!((extra.catalog_page(), extra.catalog_slot()), (4, 2));
            assert_eq!(extra.raw(), Some(raw.as_slice()));
            let properties = extra.groups()[0].properties().unwrap();
            assert_eq!(properties[0].name(), "StoredRule");
            assert_eq!(properties[0].raw(), value);
            assert_eq!(properties[0].text(), Some(source.xml));
            assert_eq!(image.as_bytes(), original);
            assert_eq!(bytes, original);
        }
    }
}

fn carrier(source: &program_fixtures::SourceGolden, code: u8) -> Vec<u8> {
    let raw = extra_fixture::bag(&[
        extra_fixture::names(&["StoredRule"]),
        extra_fixture::group(
            3,
            &[extra_fixture::property(
                0,
                1,
                12,
                &extra_fixture::text(source.xml),
            )],
        ),
    ]);
    let mut bytes = local_table(payload_fixture::inline(&raw), false);
    assert_eq!(bytes[20], 6);
    bytes[20] = code;
    bytes
}

fn section(text: &str, header: &str) -> Vec<Vec<String>> {
    let columns = header.split('\t').collect::<Vec<_>>();
    let mut reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut selected = false;
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.unwrap();
        if record.iter().eq(columns.iter().copied()) {
            assert!(!selected);
            selected = true;
            continue;
        }
        if selected {
            if record.len() != columns.len() {
                break;
            }
            rows.push(record.iter().map(str::to_owned).collect());
        }
    }
    assert!(selected, "missing program report section");
    rows
}

#[test]
fn program_section_reader_preserves_quoted_tabs_and_newlines() {
    let input = "A\tB\n\"one\tvalue\"\t\"two\nlines\"\nSummary\t0\textra\n";
    assert_eq!(
        section(input, "A\tB"),
        vec![vec!["one\tvalue", "two\nlines"]]
    );
}

fn coordinates(span: Option<(usize, usize)>) -> [String; 2] {
    span.map(|(start, end)| [start.to_string(), end.to_string()])
        .unwrap_or_default()
}

fn expected_nodes(source: &program_fixtures::SourceGolden) -> Vec<Vec<String>> {
    source
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let condition = coordinates(node.content);
            let number = coordinates(node.number);
            let description = coordinates(node.description);
            vec![
                "3".into(),
                "StoredRule".into(),
                node.definition.to_string(),
                index.to_string(),
                node.parent
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                node.kind.into(),
                node.span.0.to_string(),
                node.span.1.to_string(),
                condition[0].clone(),
                condition[1].clone(),
                number[0].clone(),
                number[1].clone(),
                description[0].clone(),
                description[1].clone(),
                node.diagnostic.unwrap_or("").into(),
            ]
        })
        .collect()
}

#[test]
fn explicit_macro_program_cli_matches_all_prefrozen_node_and_span_rows() {
    for code in [5, 6] {
        for source in program_fixtures::sources() {
            let database = super::Database::new(carrier(source, code));
            let output = super::cli_support::run(&[
                "inspect",
                database.path(),
                "--table",
                "Items",
                "--extra",
                "--macro-program",
                "--format",
                "tsv",
            ]);
            let coverage = match source.outcome {
                Outcome::RecognizedDesktopSubset => {
                    "Recognized desktop subset; expressions unevaluated"
                }
                Outcome::ContainsOpaqueStructure => {
                    "Contains opaque structure; expressions unevaluated"
                }
                Outcome::Malformed | Outcome::Unsupported => {
                    assert_eq!(output.status.code(), Some(2), "{}", source.name);
                    assert!(output.stdout.is_empty());
                    assert!(!output.stderr.is_empty());
                    continue;
                }
            };
            assert!(
                output.status.success(),
                "{}: {}",
                source.name,
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(output.stderr.is_empty());
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(text.contains("Data-macro absence\tNot certified\n"));
            assert!(text.contains("Program execution\tNot provided\n"));
            assert!(text.contains(
                "Program span coordinates\tOriginal decoded XML UTF-8 bytes; BOM retained\n"
            ));
            assert!(text.contains(&format!("Program definitions\t{}\n", source.definitions)));
            assert!(text.contains(&format!("Program nodes\t{}\n", source.nodes.len())));
            assert!(text.contains("Program documents inspected\t1\n"));
            let rows = section(
                &text,
                "Group kind\tProperty\tDefinition\tNode\tParent\tKind\tXML start\tXML end\tCondition start\tCondition end\tNumber start\tNumber end\tDescription start\tDescription end\tOpaque diagnostic",
            );
            assert_eq!(rows, expected_nodes(source), "{}", source.name);
            let namespace = if source.name == "april_simple"
                || source.name == "named_and_unknown_event_empty_bodies"
            {
                "http://schemas.microsoft.com/office/accessservices/2009/04/application"
            } else {
                "http://schemas.microsoft.com/office/accessservices/2009/11/application"
            };
            let identities = match source.name {
                "named_and_unknown_event_empty_bodies" => {
                    vec![("", "NamedCheck"), ("FutureEvent", "")]
                }
                "bom_alias_unicode_cdata" => vec![("BeforeDelete", "")],
                _ => vec![("BeforeChange", "")],
            };
            let expected = identities
                .iter()
                .enumerate()
                .map(|(index, (event, name))| {
                    vec![
                        "3".into(),
                        "StoredRule".into(),
                        index.to_string(),
                        namespace.into(),
                        (*event).into(),
                        (*name).into(),
                        coverage.into(),
                    ]
                })
                .collect::<Vec<Vec<String>>>();
            assert_eq!(
                section(
                    &text,
                    "Group kind\tProperty\tDefinition\tXML namespace\tEvent\tMacro name\tDocument structural coverage"
                ),
                expected
            );
            assert!(!text.contains("local guard"));
            assert!(!text.contains("not a numeric proof"));
            assert!(!text.contains("FutureAction"));
        }
    }
}

#[test]
fn macro_program_flags_refuse_before_open_and_low_native_pools_emit_no_stdout() {
    for flags in [
        vec!["--macro-program"],
        vec!["--extra", "--macro-program"],
        vec![
            "--table",
            "Items",
            "--extra",
            "--macro-program",
            "--macro-xml",
        ],
        vec![
            "--table",
            "Items",
            "--extra",
            "--macro-program",
            "--indexes",
        ],
    ] {
        let mut arguments = vec!["inspect", "never-open-program-fixture.accdb"];
        arguments.extend(flags);
        let output = super::cli_support::run(&arguments);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains("required") || error.contains("cannot be used"));
        assert!(!error.contains("database I/O"));
    }
    let source = &program_fixtures::sources()[0];
    let database = super::Database::new(carrier(source, 6));
    for flag in ["--native-bytes", "--native-steps"] {
        let output = super::cli_support::run(&[
            "inspect",
            database.path(),
            "--table",
            "Items",
            "--extra",
            "--macro-program",
            flag,
            "1",
        ]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn later_illformed_program_never_publishes_earlier_structural_rows() {
    let sources = program_fixtures::sources();
    let valid = sources
        .iter()
        .find(|source| source.name == "april_simple")
        .unwrap();
    let bad = sources
        .iter()
        .find(|source| source.name == "xml_unclosed")
        .unwrap();
    let raw = extra_fixture::bag(&[
        extra_fixture::names(&["StoredRule", "OtherRule"]),
        extra_fixture::group(
            3,
            &[
                extra_fixture::property(0, 1, 12, &extra_fixture::text(valid.xml)),
                extra_fixture::property(1, 1, 12, &extra_fixture::text(bad.xml)),
            ],
        ),
    ]);
    let database = super::Database::new(local_table(payload_fixture::inline(&raw), false));
    let output = super::cli_support::run(&[
        "inspect",
        database.path(),
        "--table",
        "Items",
        "--extra",
        "--macro-program",
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}
