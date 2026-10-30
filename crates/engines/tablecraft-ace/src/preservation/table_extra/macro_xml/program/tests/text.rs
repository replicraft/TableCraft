use super::{MacroProgramProfile, budget, property, text_sources::fixtures};
use crate::{InspectError, MacroProgramNodeKind, MacroProgramTextKind};
use std::cell::Cell;
use tablecraft_core::limits::ResourceBudget;

#[test]
fn macro_program_text_matches_independent_literals_and_retains_raw_spans() {
    for case in fixtures::CASES {
        for bom in [false, true] {
            let xml = fixtures::document(case.raw, bom);
            let property = property(&xml);
            let raw_property = property.raw().to_vec();
            let program = property
                .inspect_macro_program(
                    MacroProgramProfile::DocumentedDesktop,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();

            let mut extracted = 0;
            for (index, node) in program.nodes().iter().enumerate() {
                let slots: &[_] = match node.kind() {
                    MacroProgramNodeKind::Condition { .. } => &[MacroProgramTextKind::Condition],
                    MacroProgramNodeKind::RaiseError { .. } => &[
                        MacroProgramTextKind::Number,
                        MacroProgramTextKind::Description,
                    ],
                    _ => &[],
                };
                for slot in slots {
                    let text = program
                        .text_content(index, *slot, &mut budget(), &|| false)
                        .unwrap()
                        .unwrap();
                    assert_eq!(text.text(), case.decoded, "{} / BOM {bom}", case.name);
                    assert_eq!(
                        program.source_span(text.source_span()).unwrap(),
                        case.raw,
                        "{} / BOM {bom}",
                        case.name
                    );
                    assert!(text.text().len() <= case.raw.len());
                    extracted += 1;
                }
            }
            assert_eq!(extracted, 3);
            assert_eq!(program.xml_source(), xml);
            assert_eq!(program.native_property_bytes(), raw_property);
        }
    }
}

#[test]
fn macro_program_text_preserves_absent_empty_and_rejects_wrong_or_opaque_slots() {
    for present in [false, true] {
        let description = if present {
            "<Argument Name=\"Description\"/>"
        } else {
            ""
        };
        let xml = format!(
            "<DataMacro xmlns=\"http://schemas.microsoft.com/office/accessservices/2009/11/application\" Event=\"BeforeDelete\"><Statements><Action Name=\"RaiseError\"><Argument Name=\"Number\"/>{description}</Action></Statements></DataMacro>"
        );
        let property = property(&xml);
        let program = property
            .inspect_macro_program(
                MacroProgramProfile::DocumentedDesktop,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let text = program
            .text_content(1, MacroProgramTextKind::Description, &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(text.as_ref().map(|text| text.text()), present.then_some(""));
        assert_eq!(
            program
                .text_content(1, MacroProgramTextKind::Number, &mut budget(), &|| false)
                .unwrap()
                .unwrap()
                .text(),
            ""
        );
        for (node, slot, error) in [
            (0, MacroProgramTextKind::Number, InspectError::Unsupported),
            (
                1,
                MacroProgramTextKind::Condition,
                InspectError::Unsupported,
            ),
            (
                usize::MAX,
                MacroProgramTextKind::Number,
                InspectError::Malformed,
            ),
        ] {
            assert_eq!(
                program
                    .text_content(node, slot, &mut budget(), &|| false)
                    .err(),
                Some(error)
            );
        }
    }

    let property = property(&fixtures::document("<Unknown/>", false));
    let program = property
        .inspect_macro_program(
            MacroProgramProfile::DocumentedDesktop,
            &mut budget(),
            &|| false,
        )
        .unwrap();
    let opaque = program
        .nodes()
        .iter()
        .position(|node| matches!(node.kind(), MacroProgramNodeKind::Opaque { .. }))
        .unwrap();
    for slot in [
        MacroProgramTextKind::Condition,
        MacroProgramTextKind::Number,
        MacroProgramTextKind::Description,
    ] {
        assert_eq!(
            program
                .text_content(opaque, slot, &mut budget(), &|| false)
                .err(),
            Some(InspectError::Unsupported)
        );
    }
}

#[test]
fn macro_program_text_obeys_shared_pools_and_final_cancellation() {
    let property = property(&fixtures::document("a&#13;<![CDATA[b\r\nc]]>&amp;d", true));
    let program = property
        .inspect_macro_program(
            MacroProgramProfile::DocumentedDesktop,
            &mut budget(),
            &|| false,
        )
        .unwrap();
    let node = program
        .nodes()
        .iter()
        .position(|node| matches!(node.kind(), MacroProgramNodeKind::Condition { .. }))
        .unwrap();
    assert!(matches!(
        program
            .text_content(
                node,
                MacroProgramTextKind::Condition,
                &mut ResourceBudget::new(0, 0, 100_000),
                &|| false,
            )
            .err(),
        Some(InspectError::Limit(_))
    ));
    assert!(matches!(
        program
            .text_content(
                node,
                MacroProgramTextKind::Condition,
                &mut ResourceBudget::new(100_000, 0, 0),
                &|| false,
            )
            .err(),
        Some(InspectError::Limit(_))
    ));
    assert_eq!(
        program
            .text_content(
                node,
                MacroProgramTextKind::Condition,
                &mut budget(),
                &|| true
            )
            .err(),
        Some(InspectError::Cancelled)
    );

    let polls = Cell::new(0_usize);
    let text = program
        .text_content(
            node,
            MacroProgramTextKind::Condition,
            &mut budget(),
            &|| {
                polls.set(polls.get() + 1);
                false
            },
        )
        .unwrap()
        .unwrap();
    assert_eq!(text.text(), "a\rb\nc&d");
    let healthy_polls = polls.get();
    assert!(healthy_polls > 3);

    polls.set(0);
    assert_eq!(
        program
            .text_content(
                node,
                MacroProgramTextKind::Condition,
                &mut budget(),
                &|| {
                    polls.set(polls.get() + 1);
                    polls.get() == healthy_polls
                }
            )
            .err(),
        Some(InspectError::Cancelled)
    );
    assert_eq!(polls.get(), healthy_polls);
}

#[test]
fn macro_program_text_retains_multiple_leading_feff_and_bounded_large_output() {
    let raw = format!("\u{feff}\u{feff}{}", "z".repeat(65_536));
    let property = property(&fixtures::document(&raw, true));
    let program = property
        .inspect_macro_program(
            MacroProgramProfile::DocumentedDesktop,
            &mut budget(),
            &|| false,
        )
        .unwrap();
    let node = program
        .nodes()
        .iter()
        .position(|node| matches!(node.kind(), MacroProgramNodeKind::Condition { .. }))
        .unwrap();
    let text = program
        .text_content(
            node,
            MacroProgramTextKind::Condition,
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .unwrap();
    assert_eq!(text.text(), raw);
    assert_eq!(program.source_span(text.source_span()).unwrap(), raw);
}
