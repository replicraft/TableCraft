use super::budget;
use crate::{
    InspectError, MacroProgramCoverage, MacroProgramNodeKind, MacroProgramProfile, NativeImage,
    PayloadPresence, TableExtraCoverage, read_table,
};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

pub(crate) mod model;

#[test]
fn qualify_explicit_guard_property_selection_sources_before_execution() {
    let mut admitted = 0;
    let mut refused = 0;
    for code in [5, 6] {
        for variant in [
            model::Variant::Valid,
            model::Variant::SecondOpaque,
            model::Variant::SecondMalformed,
            model::Variant::DuplicateKind,
        ] {
            let source = model::source(code, variant);
            let scope = CatalogScope::new(7);
            let image = NativeImage::capture(
                &mut Cursor::new(&source.bytes),
                scope,
                &mut budget(),
                &|| false,
            )
            .unwrap_or_else(|error| panic!("{variant:?} / header {code}: capture {error}"));
            let table = read_table(&mut image.reader(), scope, "Items", &mut budget(), &|| {
                false
            })
            .unwrap_or_else(|error| panic!("{variant:?} / header {code}: read rows {error}"));
            assert_eq!(
                table.rows(),
                [
                    vec![Value::Int32(1), Value::Int32(10)],
                    vec![Value::Int32(2), Value::Int32(20)],
                    vec![Value::Int32(3), Value::Int32(30)]
                ]
            );
            assert_eq!(table.schema().object_id(), scope.object_id(10));
            let payloads = image
                .inspect_object_payloads(scope.object_id(10), &mut budget(), &|| false)
                .unwrap_or_else(|error| {
                    panic!("{variant:?} / header {code}: payload presence {error}")
                });
            assert_eq!(payloads.body(), PayloadPresence::Absent);
            assert_eq!(payloads.module(), PayloadPresence::Absent);
            assert_eq!(payloads.extra(), PayloadPresence::Present);
            let result = image.inspect_table_extra(scope.object_id(10), &mut budget(), &|| false);
            if matches!(variant, model::Variant::DuplicateKind) {
                assert_eq!(
                    result.err(),
                    Some(InspectError::Malformed),
                    "duplicate parsed group kind / header {code}"
                );
                assert!(
                    image.as_bytes() == source.bytes,
                    "{variant:?} / header {code}: source changed"
                );
                refused += 1;
                continue;
            }
            let extra = result
                .unwrap_or_else(|error| panic!("{variant:?} / header {code}: read Extra {error}"));
            admitted += 1;
            assert_eq!(extra.coverage(), TableExtraCoverage::WideMr2);
            assert_eq!(extra.raw(), Some(source.bag.as_slice()));
            assert!(extra.groups().iter().all(|group| group.kind() == 3));
            let properties = extra
                .groups()
                .iter()
                .flat_map(|group| group.properties().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(properties.len(), 2, "{variant:?} / header {code}");
            assert_eq!(properties[0].name(), "GuardA");
            assert_eq!(properties[0].text(), Some(source.alpha.as_str()));
            assert_eq!(properties[1].text(), Some(source.beta.as_str()));
            for (property, xml) in [
                (properties[0], source.alpha.as_str()),
                (properties[1], source.beta.as_str()),
            ] {
                assert_eq!(
                    property.raw(),
                    xml.encode_utf16()
                        .flat_map(u16::to_le_bytes)
                        .collect::<Vec<_>>()
                );
            }
            assert_eq!(
                (properties[0].native_type(), properties[0].flags()),
                (12, 1)
            );
            assert_eq!(
                (properties[1].native_type(), properties[1].flags()),
                (12, 1)
            );
            let alpha = properties[0]
                .inspect_macro_program(
                    MacroProgramProfile::DocumentedDesktop,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(
                alpha.coverage(),
                MacroProgramCoverage::RecognizedDesktopSubset
            );
            assert_eq!(
                alpha
                    .nodes()
                    .iter()
                    .filter(|node| matches!(node.kind(), MacroProgramNodeKind::RaiseError { .. }))
                    .count(),
                1
            );
            let MacroProgramNodeKind::RaiseError {
                number,
                description,
            } = alpha.nodes().last().unwrap().kind()
            else {
                panic!("expected alpha RaiseError");
            };
            assert_eq!(alpha.source_span(number).unwrap(), "71");
            assert_eq!(alpha.source_span(description.unwrap()).unwrap(), "alpha");
            match variant {
                model::Variant::Valid => {
                    assert_eq!(properties[1].name(), "GuardB");
                    assert_eq!(
                        properties[1]
                            .inspect_macro_program(
                                MacroProgramProfile::DocumentedDesktop,
                                &mut budget(),
                                &|| false
                            )
                            .unwrap()
                            .coverage(),
                        MacroProgramCoverage::RecognizedDesktopSubset
                    );
                    let beta = properties[1]
                        .inspect_macro_program(
                            MacroProgramProfile::DocumentedDesktop,
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap();
                    let MacroProgramNodeKind::RaiseError {
                        number,
                        description,
                    } = beta.nodes().last().unwrap().kind()
                    else {
                        panic!("expected beta RaiseError");
                    };
                    assert_eq!(beta.source_span(number).unwrap(), "73");
                    assert_eq!(beta.source_span(description.unwrap()).unwrap(), "beta");
                }
                model::Variant::SecondOpaque => {
                    assert_eq!(properties[1].name(), "GuardB");
                    assert_eq!(
                        properties[1]
                            .inspect_macro_program(
                                MacroProgramProfile::DocumentedDesktop,
                                &mut budget(),
                                &|| false
                            )
                            .unwrap()
                            .coverage(),
                        MacroProgramCoverage::ContainsOpaqueStructure
                    );
                }
                model::Variant::SecondMalformed => {
                    assert_eq!(properties[1].name(), "GuardB");
                    assert_eq!(
                        properties[1]
                            .inspect_macro_program(
                                MacroProgramProfile::DocumentedDesktop,
                                &mut budget(),
                                &|| false
                            )
                            .err(),
                        Some(InspectError::Malformed)
                    );
                }
                model::Variant::DuplicateKind => unreachable!(),
            }
            assert!(
                image.as_bytes() == source.bytes,
                "{variant:?} / header {code}: source changed"
            );
        }
    }
    assert_eq!((admitted, refused), (6, 2));
}
