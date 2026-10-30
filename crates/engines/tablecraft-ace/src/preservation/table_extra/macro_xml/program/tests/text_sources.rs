use super::{MacroProgramCoverage, MacroProgramNodeKind, MacroProgramProfile, budget, property};

pub(super) mod fixtures;

#[test]
fn qualify_macro_program_text_sources_before_extraction() {
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
                .unwrap_or_else(|error| panic!("{} / BOM {bom}: {error}", case.name));

            assert_eq!(
                program.coverage(),
                MacroProgramCoverage::RecognizedDesktopSubset,
                "{} / BOM {bom}",
                case.name
            );
            assert_eq!(program.xml_source(), xml);
            assert_eq!(program.native_property_bytes(), raw_property);

            let mut slots = 0;
            for node in program.nodes() {
                match node.kind() {
                    MacroProgramNodeKind::Condition { content } => {
                        assert_eq!(program.source_span(content).unwrap(), case.raw);
                        slots += 1;
                    }
                    MacroProgramNodeKind::RaiseError {
                        number,
                        description,
                    } => {
                        assert_eq!(program.source_span(number).unwrap(), case.raw);
                        assert_eq!(program.source_span(description.unwrap()).unwrap(), case.raw);
                        slots += 2;
                    }
                    _ => {}
                }
            }
            assert_eq!(slots, 3, "{} / BOM {bom}", case.name);
            // Expected decoded values are independent literal data, retained
            // before any extraction implementation or API has run.
            assert!(case.decoded.len() <= case.raw.len());
        }
    }
}

#[test]
fn qualify_macro_program_text_slot_boundaries_before_extraction() {
    for argument in [
        "<Argument Name=\"Number\"/>",
        "<Argument Name=\"Number\"/><Argument Name=\"Description\"/>",
        "<Argument Name=\"Number\"/><Argument Name=\"Description\"></Argument>",
    ] {
        let xml = format!(
            "<DataMacro xmlns=\"http://schemas.microsoft.com/office/accessservices/2009/11/application\" Event=\"BeforeDelete\"><Statements><Action Name=\"RaiseError\">{argument}</Action></Statements></DataMacro>"
        );
        let property = property(&xml);
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
        let node = program.nodes().last().unwrap();
        let MacroProgramNodeKind::RaiseError {
            number,
            description,
        } = node.kind()
        else {
            panic!("expected recognized RaiseError");
        };
        assert_eq!(program.source_span(number).unwrap(), "");
        assert_eq!(description.is_some(), argument.contains("Description"));
        if let Some(description) = description {
            assert_eq!(program.source_span(description).unwrap(), "");
        }
    }

    for raw in ["<Unknown/>", "<x:Field xmlns:x=\"urn:other\"/>"] {
        let xml = fixtures::document(raw, false);
        let property = property(&xml);
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
        assert!(program.nodes().iter().all(|node| !matches!(
            node.kind(),
            MacroProgramNodeKind::Condition { .. } | MacroProgramNodeKind::RaiseError { .. }
        )));
    }
}
