use super::{MacroProgramCoverage, MacroProgramNodeKind, MacroProgramProfile};
use crate::{
    InspectError,
    preservation::table_extra::{
        TableExtraProperty,
        program_fixtures::{Outcome, sources},
    },
};
use std::cell::Cell;
use tablecraft_core::limits::{Resource, ResourceBudget};

mod text;
mod text_sources;

const NAMESPACE: &str = "http://schemas.microsoft.com/office/accessservices/2009/11/application";

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 0, 100_000_000)
}

fn property(text: &str) -> TableExtraProperty {
    TableExtraProperty {
        name: "StoredRule".into(),
        native_type: 12,
        flags: 1,
        raw: text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
        text: Some(text.into()),
    }
}

#[test]
fn documented_desktop_program_matches_all_frozen_structures_and_original_spans() {
    for source in sources() {
        let property = property(source.xml);
        let original_raw = property.raw().to_vec();
        let result = property.inspect_macro_program(
            MacroProgramProfile::DocumentedDesktop,
            &mut budget(),
            &|| false,
        );
        let coverage = match source.outcome {
            Outcome::RecognizedDesktopSubset => MacroProgramCoverage::RecognizedDesktopSubset,
            Outcome::ContainsOpaqueStructure => MacroProgramCoverage::ContainsOpaqueStructure,
            Outcome::Malformed => {
                assert_eq!(
                    result.err(),
                    Some(InspectError::Malformed),
                    "{}",
                    source.name
                );
                continue;
            }
            Outcome::Unsupported => {
                assert_eq!(
                    result.err(),
                    Some(InspectError::Unsupported),
                    "{}",
                    source.name
                );
                continue;
            }
        };
        let program = result.unwrap();
        assert_eq!(program.profile(), MacroProgramProfile::DocumentedDesktop);
        assert_eq!(program.coverage(), coverage, "{}", source.name);
        assert_eq!(program.xml_source(), source.xml);
        assert_eq!(program.native_property_bytes(), original_raw);
        assert_eq!(program.envelopes().definitions().len(), source.definitions);
        assert_eq!(program.nodes().len(), source.nodes.len(), "{}", source.name);
        for (actual, expected) in program.nodes().iter().zip(source.nodes) {
            assert_eq!(actual.definition(), expected.definition);
            assert_eq!(actual.parent(), expected.parent);
            assert_eq!(
                (actual.span().start(), actual.span().end()),
                expected.span,
                "{}",
                source.name
            );
            assert_eq!(
                program.source_span(actual.span()).unwrap().as_bytes(),
                &source.xml.as_bytes()[expected.span.0..expected.span.1]
            );
            let (kind, content, number, description, diagnostic) = match actual.kind() {
                MacroProgramNodeKind::Statements => ("Statements", None, None, None, None),
                MacroProgramNodeKind::ConditionalBlock => {
                    ("ConditionalBlock", None, None, None, None)
                }
                MacroProgramNodeKind::If => ("If", None, None, None, None),
                MacroProgramNodeKind::Condition { content } => {
                    ("Condition", Some(content), None, None, None)
                }
                MacroProgramNodeKind::RaiseError {
                    number,
                    description,
                } => ("RaiseError", None, Some(number), description, None),
                MacroProgramNodeKind::Opaque { diagnostic } => {
                    ("Opaque", None, None, None, Some(diagnostic))
                }
            };
            assert_eq!(kind, expected.kind);
            assert_eq!(
                content.map(|span| (span.start(), span.end())),
                expected.content
            );
            assert_eq!(
                number.map(|span| (span.start(), span.end())),
                expected.number
            );
            assert_eq!(
                description.map(|span| (span.start(), span.end())),
                expected.description
            );
            assert_eq!(
                diagnostic.map(|value| format!("{value:?}")),
                expected.diagnostic.map(str::to_owned)
            );
        }
        assert_eq!(property.raw(), original_raw);
        assert_eq!(property.text(), Some(source.xml));
    }
}

#[test]
fn empty_present_arguments_and_unevaluated_values_do_not_become_type_validation() {
    for name in [
        "present_empty_arguments",
        "number_expression_not_typed",
        "present_empty_condition",
    ] {
        let source = sources().iter().find(|source| source.name == name).unwrap();
        let property = property(source.xml);
        let program = property
            .inspect_macro_program(
                MacroProgramProfile::DocumentedDesktop,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(
            program.coverage(),
            MacroProgramCoverage::RecognizedDesktopSubset
        );
        if name == "present_empty_arguments" {
            let MacroProgramNodeKind::RaiseError {
                number,
                description,
            } = program.nodes()[1].kind()
            else {
                panic!("expected literal RaiseError shape");
            };
            assert_eq!(program.source_span(number).unwrap(), "");
            assert_eq!(program.source_span(description.unwrap()).unwrap(), "");
        }
    }
}

#[test]
fn envelope_only_namespace_behavior_stays_separate_from_program_opaque_coverage() {
    for name in [
        "foreign_known_local_name",
        "prefix_rebinding",
        "foreign_argument_element",
    ] {
        let source = sources().iter().find(|source| source.name == name).unwrap();
        let property = property(source.xml);
        assert_eq!(
            property.inspect_macro_xml(&mut budget(), &|| false).err(),
            Some(InspectError::Unsupported)
        );
        let program = property
            .inspect_macro_program(
                MacroProgramProfile::DocumentedDesktop,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(
            program.coverage(),
            MacroProgramCoverage::ContainsOpaqueStructure
        );
        assert_eq!(program.envelopes().namespace().uri(), NAMESPACE);
    }
}

#[test]
fn program_bounds_and_budget_failures_return_no_partial_ast() {
    let source = sources()
        .iter()
        .find(|source| source.name == "april_simple")
        .unwrap();
    let original = property(source.xml);
    for mut pool in [
        ResourceBudget::new(0, 0, 100_000_000),
        ResourceBudget::new(128 * 1024 * 1024, 0, 0),
    ] {
        assert!(matches!(
            original.inspect_macro_program(
                MacroProgramProfile::DocumentedDesktop,
                &mut pool,
                &|| false
            ),
            Err(InspectError::Limit(_))
        ));
    }
    let mut pool = budget();
    original
        .inspect_macro_program(MacroProgramProfile::DocumentedDesktop, &mut pool, &|| false)
        .unwrap();
    assert_eq!(pool.remaining(Resource::Rows), 0);

    let over_input = " ".repeat(2_097_153);
    let deep = format!(
        "<DataMacro xmlns=\"{NAMESPACE}\" Event=\"BeforeChange\">{}{}</DataMacro>",
        "<Future>".repeat(129),
        "</Future>".repeat(129)
    );
    let arguments = format!(
        "<DataMacro xmlns=\"{NAMESPACE}\" Event=\"BeforeChange\"><Statements><Action Name=\"FutureAction\">{}</Action></Statements></DataMacro>",
        "<Argument Name=\"Future\"/>".repeat(65)
    );
    let nodes = format!(
        "<DataMacro xmlns=\"{NAMESPACE}\" Event=\"BeforeChange\"><Statements>{}</Statements></DataMacro>",
        "<Action Name=\"RaiseError\"><Argument Name=\"Number\"/></Action>".repeat(4096)
    );
    for text in [over_input, deep, arguments, nodes] {
        assert_eq!(
            property(&text)
                .inspect_macro_program(
                    MacroProgramProfile::DocumentedDesktop,
                    &mut budget(),
                    &|| false
                )
                .err(),
            Some(InspectError::StructureLimit)
        );
    }
}

#[test]
fn program_early_and_measured_terminal_cancellation_publish_no_partial_result() {
    let source = sources()
        .iter()
        .find(|source| source.name == "bom_alias_unicode_cdata")
        .unwrap();
    let property = property(source.xml);
    assert_eq!(
        property
            .inspect_macro_program(
                MacroProgramProfile::DocumentedDesktop,
                &mut budget(),
                &|| true
            )
            .err(),
        Some(InspectError::Cancelled)
    );
    let healthy = Cell::new(0);
    property
        .inspect_macro_program(
            MacroProgramProfile::DocumentedDesktop,
            &mut budget(),
            &|| {
                healthy.set(healthy.get() + 1);
                false
            },
        )
        .unwrap();
    let calls = Cell::new(0);
    assert_eq!(
        property
            .inspect_macro_program(
                MacroProgramProfile::DocumentedDesktop,
                &mut budget(),
                &|| {
                    calls.set(calls.get() + 1);
                    calls.get() == healthy.get()
                }
            )
            .err(),
        Some(InspectError::Cancelled)
    );
    assert_eq!(property.text(), Some(source.xml));
}
