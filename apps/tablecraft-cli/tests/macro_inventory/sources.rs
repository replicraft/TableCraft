use super::inspect;
use tablecraft_ace::{InspectError, TableExtraCoverage};

#[path = "sources/api.rs"]
mod api;

#[path = "../../../../crates/engines/tablecraft-ace/src/preservation/table_extra/tests/observed_macro_inventory/model.rs"]
mod model;

const NAMESPACE: &str = "http://schemas.microsoft.com/office/accessservices/2009/11/application";

fn expected(case: model::Case) -> Vec<Vec<&'static str>> {
    let event = vec![
        "0",
        "3",
        "0",
        "ValidateLedger",
        "Observed envelope",
        NAMESPACE,
        "0",
        "BeforeChange",
        "",
    ];
    let named = vec![
        "0",
        "4",
        "0",
        "RecomputeBalance",
        "Observed envelope",
        NAMESPACE,
        "0",
        "",
        "RecomputeBalance",
    ];
    match case {
        model::Case::NonstandardEventProperty => vec![event],
        model::Case::NamedProgram => vec![named],
        model::Case::EventAndNamed => {
            let mut second = named;
            second[0] = "1";
            vec![event, second]
        }
        model::Case::NamesOnly | model::Case::OpaqueExtra => Vec::new(),
        model::Case::OpaqueGroup => vec![vec![
            "0",
            "99",
            "",
            "",
            "Unresolved: opaque group",
            "",
            "",
            "",
            "",
        ]],
        model::Case::MalformedXml => vec![vec![
            "0",
            "3",
            "0",
            "ValidateLedger",
            "Unresolved: malformed XML",
            "",
            "",
            "",
            "",
        ]],
        model::Case::NonTextProperty => vec![vec![
            "0",
            "3",
            "0",
            "ValidateLedger",
            "Unresolved: no qualified text",
            "",
            "",
            "",
            "",
        ]],
        model::Case::MultipleEnvelopes => vec![
            vec![
                "0",
                "3",
                "0",
                "EnvelopeSet",
                "Observed envelope",
                NAMESPACE,
                "0",
                "BeforeDelete",
                "",
            ],
            vec![
                "0",
                "3",
                "0",
                "EnvelopeSet",
                "Observed envelope",
                NAMESPACE,
                "1",
                "",
                "Repair",
            ],
        ],
        model::Case::UnfamiliarEvent => vec![vec![
            "0",
            "3",
            "0",
            "ValidateLedger",
            "Observed envelope",
            NAMESPACE,
            "0",
            "FutureEvent",
            "",
        ]],
    }
}

#[test]
fn qualify_observed_macro_inventory_cli_sources_before_commands() {
    let mut carriers = 0;
    let mut documents = 0;
    let mut definitions = 0;
    for code in [5, 6] {
        for case in model::CASES {
            let source = model::source(code, case);
            let image = inspect::source(&source.bytes);
            let extra = inspect::extra(&image).unwrap();
            assert_eq!(extra.raw(), Some(source.bag.as_slice()));
            let opaque = matches!(
                case,
                model::Case::OpaqueExtra | model::Case::OpaqueGroup | model::Case::NonTextProperty
            );
            assert_eq!(
                extra.coverage(),
                if opaque {
                    TableExtraCoverage::Opaque
                } else {
                    TableExtraCoverage::WideMr2
                }
            );
            let mut observed = Vec::new();
            for (group_ordinal, group) in extra.groups().iter().enumerate() {
                let group_number = group_ordinal.to_string();
                let kind = group.kind().to_string();
                let Some(properties) = group.properties() else {
                    observed.push(vec![
                        group_number,
                        kind,
                        "".into(),
                        "".into(),
                        "Unresolved: opaque group".into(),
                        "".into(),
                        "".into(),
                        "".into(),
                        "".into(),
                    ]);
                    continue;
                };
                for (property_ordinal, property) in properties.iter().enumerate() {
                    let prefix = [
                        group_number.clone(),
                        kind.clone(),
                        property_ordinal.to_string(),
                        property.name().into(),
                    ];
                    let parsed = property.inspect_macro_xml(&mut inspect::budget(), &|| false);
                    match parsed {
                        Ok(xml) => {
                            documents += 1;
                            for (ordinal, definition) in xml.definitions().iter().enumerate() {
                                observed.push(vec![
                                    prefix[0].clone(),
                                    prefix[1].clone(),
                                    prefix[2].clone(),
                                    prefix[3].clone(),
                                    "Observed envelope".into(),
                                    xml.namespace().uri().into(),
                                    ordinal.to_string(),
                                    definition.event().unwrap_or("").into(),
                                    definition.name().unwrap_or("").into(),
                                ]);
                                definitions += 1;
                            }
                        }
                        Err(error) => {
                            let status = if matches!(case, model::Case::NonTextProperty) {
                                assert_eq!(error, InspectError::Unsupported);
                                assert_eq!((property.native_type(), property.flags()), (4, 0));
                                assert_eq!(property.raw(), [17, 0, 0, 0]);
                                "Unresolved: no qualified text"
                            } else {
                                assert!(matches!(case, model::Case::MalformedXml));
                                assert_eq!(error, InspectError::Malformed);
                                "Unresolved: malformed XML"
                            };
                            observed.push(vec![
                                prefix[0].clone(),
                                prefix[1].clone(),
                                prefix[2].clone(),
                                prefix[3].clone(),
                                status.into(),
                                "".into(),
                                "".into(),
                                "".into(),
                                "".into(),
                            ]);
                        }
                    }
                }
            }
            assert_eq!(observed, expected(case), "{case:?}/{code}");
            assert_eq!(image.as_bytes(), source.bytes);
            carriers += 1;
        }
    }
    assert_eq!((carriers, documents, definitions), (20, 12, 14));
}
