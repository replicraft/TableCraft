use super::{super::budget, model};
use crate::{InspectError, MacroPropertyObservation, NativeImage, TableExtraCoverage};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

fn image(code: u8, case: model::Case) -> NativeImage {
    let source = model::source(code, case);
    NativeImage::capture(
        &mut Cursor::new(source.bytes),
        CatalogScope::new(7),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn observed_inventory_preserves_source_identity_and_all_literal_envelopes() {
    let mut documents = 0;
    let mut definitions = 0;
    for code in [5, 6] {
        for case in model::CASES {
            let image = image(code, case);
            let object = CatalogScope::new(7).object_id(10);
            let extra = image
                .inspect_table_extra(object, &mut budget(), &|| false)
                .unwrap();
            let inventory = extra
                .inspect_macro_inventory(&mut budget(), &|| false)
                .unwrap();
            assert!(std::ptr::eq(inventory.source(), &extra));
            assert_eq!(inventory.source().object_id(), object);
            assert_eq!(inventory.source().native_id(), 10);
            assert_eq!(inventory.source().parent_id(), 0);
            assert_eq!(
                (
                    inventory.source().catalog_page(),
                    inventory.source().catalog_slot()
                ),
                (4, 2)
            );

            let expected_groups: &[usize] = if matches!(case, model::Case::OpaqueGroup) {
                &[0]
            } else {
                &[]
            };
            assert_eq!(
                inventory.opaque_groups(),
                expected_groups,
                "{case:?}/{code}"
            );
            let expected_names: &[usize] = match case {
                model::Case::NamedProgram | model::Case::OpaqueGroup => &[0],
                model::Case::NamesOnly => &[0, 1],
                _ => &[],
            };
            assert_eq!(
                inventory.unrepresented_names(),
                expected_names,
                "{case:?}/{code}"
            );
            let expected_properties = match case {
                model::Case::NamesOnly | model::Case::OpaqueExtra | model::Case::OpaqueGroup => 0,
                model::Case::EventAndNamed => 2,
                _ => 1,
            };
            assert_eq!(
                inventory.properties().len(),
                expected_properties,
                "{case:?}/{code}"
            );
            for (ordinal, property) in inventory.properties().iter().enumerate() {
                assert_eq!(property.group_ordinal(), ordinal);
                assert_eq!(property.property_ordinal(), 0);
                match property.observation() {
                    MacroPropertyObservation::NoText => {
                        assert!(matches!(case, model::Case::NonTextProperty));
                        assert_eq!(inventory.source().coverage(), TableExtraCoverage::Opaque);
                    }
                    MacroPropertyObservation::MalformedXml => {
                        assert!(matches!(case, model::Case::MalformedXml));
                    }
                    MacroPropertyObservation::UnsupportedXml => {
                        panic!("unexpected {case:?}/{code}")
                    }
                    MacroPropertyObservation::Envelopes(xml) => {
                        let actual = xml
                            .definitions()
                            .iter()
                            .map(|definition| (definition.event(), definition.name()))
                            .collect::<Vec<_>>();
                        let named = matches!(case, model::Case::NamedProgram)
                            || matches!(case, model::Case::EventAndNamed) && ordinal == 1;
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
                        assert_eq!(actual, expected, "{case:?}/{code}");
                        documents += 1;
                        definitions += actual.len();
                    }
                }
            }
        }
    }
    assert_eq!((documents, definitions), (12, 14));
}

#[test]
fn observed_inventory_never_turns_cancellation_or_resource_failure_into_coverage() {
    let image = image(6, model::Case::EventAndNamed);
    let extra = image
        .inspect_table_extra(CatalogScope::new(7).object_id(10), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(
        extra.inspect_macro_inventory(&mut budget(), &|| true).err(),
        Some(InspectError::Cancelled)
    );

    let mut bytes = ResourceBudget::new(0, 16_384, 1_000_000);
    assert!(matches!(
        extra.inspect_macro_inventory(&mut bytes, &|| false),
        Err(InspectError::Limit(_))
    ));
    let mut steps = ResourceBudget::new(32 * 1024 * 1024, 16_384, 0);
    assert!(matches!(
        extra.inspect_macro_inventory(&mut steps, &|| false),
        Err(InspectError::Limit(_))
    ));

    let calls = Cell::new(0);
    let inventory = extra
        .inspect_macro_inventory(&mut budget(), &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    assert_eq!(inventory.properties().len(), 2);
    let last = calls.get();
    assert!(last > 1);
    let calls = Cell::new(0);
    assert_eq!(
        extra
            .inspect_macro_inventory(&mut budget(), &|| {
                calls.set(calls.get() + 1);
                calls.get() == last
            })
            .err(),
        Some(InspectError::Cancelled)
    );

    let mut exhausted = budget();
    exhausted.charge(Resource::Steps, 1_000_000).unwrap();
    assert!(matches!(
        extra.inspect_macro_inventory(&mut exhausted, &|| false),
        Err(InspectError::Limit(_))
    ));
}
