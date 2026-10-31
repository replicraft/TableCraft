use super::budget;
use crate::{InspectError, NativeImage, PayloadPresence, TableExtraCoverage, read_table};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

mod api;
mod model;

#[test]
fn qualify_observed_macro_inventory_sources_before_inventory() {
    let mut carriers = 0;
    let mut documents = 0;
    let mut envelopes = 0;
    for code in [5, 6] {
        for case in model::CASES {
            let source = model::source(code, case);
            let scope = CatalogScope::new(7);
            let image = NativeImage::capture(
                &mut Cursor::new(&source.bytes),
                scope,
                &mut budget(),
                &|| false,
            )
            .unwrap_or_else(|error| panic!("{case:?}/{code}: capture {error}"));
            let table = read_table(&mut image.reader(), scope, "Items", &mut budget(), &|| {
                false
            })
            .unwrap_or_else(|error| panic!("{case:?}/{code}: original rows {error}"));
            assert_eq!(
                table.rows(),
                [
                    vec![Value::Int32(1), Value::Int32(10)],
                    vec![Value::Int32(2), Value::Int32(20)],
                    vec![Value::Int32(3), Value::Int32(30)],
                ],
                "{case:?}/{code}"
            );

            let extra = image
                .inspect_table_extra(scope.object_id(10), &mut budget(), &|| false)
                .unwrap_or_else(|error| panic!("{case:?}/{code}: Extra {error}"));
            assert_eq!(extra.object_id(), scope.object_id(10));
            assert_eq!(extra.native_id(), 10);
            assert_eq!(extra.parent_id(), 0);
            assert_eq!((extra.catalog_page(), extra.catalog_slot()), (4, 2));
            assert_eq!(extra.presence(), PayloadPresence::Present);
            assert_eq!(extra.raw(), Some(source.bag.as_slice()));
            assert_eq!(
                extra.coverage(),
                if matches!(
                    case,
                    model::Case::OpaqueExtra
                        | model::Case::OpaqueGroup
                        | model::Case::NonTextProperty
                ) {
                    TableExtraCoverage::Opaque
                } else {
                    TableExtraCoverage::WideMr2
                },
                "{case:?}/{code}"
            );
            let expected_names: &[&str] = match case {
                model::Case::NamedProgram => &["UnusedName", "RecomputeBalance"],
                model::Case::EventAndNamed | model::Case::NamesOnly => {
                    &["ValidateLedger", "RecomputeBalance"]
                }
                model::Case::OpaqueExtra => &[],
                model::Case::MultipleEnvelopes => &["EnvelopeSet"],
                _ => &["ValidateLedger"],
            };
            assert_eq!(extra.names(), expected_names, "{case:?}/{code}");
            if matches!(case, model::Case::NamesOnly | model::Case::OpaqueExtra) {
                assert!(extra.groups().is_empty(), "{case:?}/{code}");
            }
            for (group_ordinal, group) in extra.groups().iter().enumerate() {
                if matches!(case, model::Case::OpaqueGroup) {
                    assert_eq!(group.kind(), 99);
                    assert_eq!(group.raw(), [0xa1, 0xb2, 0xc3]);
                    assert!(group.properties().is_none());
                    continue;
                }
                assert_eq!(
                    group.kind(),
                    if matches!(case, model::Case::NamedProgram)
                        || matches!(case, model::Case::EventAndNamed) && group_ordinal == 1
                    {
                        4
                    } else {
                        3
                    }
                );
                let properties = group.properties().unwrap();
                assert_eq!(properties.len(), 1);
                let property = &properties[0];
                let named = matches!(case, model::Case::NamedProgram)
                    || matches!(case, model::Case::EventAndNamed) && group_ordinal == 1;
                assert_eq!(
                    property.name(),
                    if named {
                        "RecomputeBalance"
                    } else if matches!(case, model::Case::MultipleEnvelopes) {
                        "EnvelopeSet"
                    } else {
                        "ValidateLedger"
                    }
                );
                if matches!(case, model::Case::NonTextProperty) {
                    assert_eq!((property.native_type(), property.flags()), (4, 0));
                    assert_eq!(property.raw(), [17, 0, 0, 0]);
                    assert!(property.text().is_none());
                    assert_eq!(
                        property.inspect_macro_xml(&mut budget(), &|| false).err(),
                        Some(InspectError::Unsupported)
                    );
                    continue;
                }
                assert_eq!((property.native_type(), property.flags()), (12, 1));
                let text = if named {
                    model::NAMED_XML
                } else {
                    match case {
                        model::Case::MalformedXml => "<broken",
                        model::Case::MultipleEnvelopes => model::WRAPPED_XML,
                        model::Case::UnfamiliarEvent => model::FUTURE_XML,
                        _ => model::EVENT_XML,
                    }
                };
                assert_eq!(property.text(), Some(text));
                assert_eq!(
                    property.raw(),
                    text.encode_utf16()
                        .flat_map(u16::to_le_bytes)
                        .collect::<Vec<_>>()
                );
                let parsed = property.inspect_macro_xml(&mut budget(), &|| false);
                if matches!(case, model::Case::MalformedXml) {
                    assert_eq!(parsed.err(), Some(InspectError::Malformed));
                    continue;
                }
                let xml = parsed.unwrap_or_else(|error| panic!("{case:?}/{code}: XML {error}"));
                assert_eq!(
                    xml.namespace().uri(),
                    "http://schemas.microsoft.com/office/accessservices/2009/11/application"
                );
                let expected: &[(Option<&str>, Option<&str>)] = if named {
                    &[(None, Some("RecomputeBalance"))]
                } else {
                    match case {
                        model::Case::MultipleEnvelopes => {
                            &[(Some("BeforeDelete"), None), (None, Some("Repair"))]
                        }
                        model::Case::UnfamiliarEvent => &[(Some("FutureEvent"), None)],
                        _ => &[(Some("BeforeChange"), None)],
                    }
                };
                let actual = xml
                    .definitions()
                    .iter()
                    .map(|definition| (definition.event(), definition.name()))
                    .collect::<Vec<_>>();
                assert_eq!(actual, expected, "{case:?}/{code}");
                documents += 1;
                envelopes += actual.len();
            }
            assert_eq!(image.as_bytes(), source.bytes, "{case:?}/{code}: mutation");
            carriers += 1;
        }
    }
    assert_eq!((carriers, documents, envelopes), (20, 12, 14));
}

mod coverage_sources;
