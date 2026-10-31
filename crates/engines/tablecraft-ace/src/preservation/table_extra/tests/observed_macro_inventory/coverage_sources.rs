//! Original-only coverage carriers, qualified without the new collector.
mod model;
mod preparation;

use super::super::{budget, guard_selector_sources};
use super::model as observed;
use crate::{
    InspectError, MacroPropertyObservation, NativeImage, PayloadPresence, TableExtraCoverage,
};
use std::io::Cursor;
use tablecraft_model::{CatalogScope, ModelError, ObjectKind};

fn capture(bytes: &[u8], scope: CatalogScope) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope, &mut budget(), &|| false).unwrap()
}

fn presence(expected: model::Presence) -> PayloadPresence {
    match expected {
        model::Presence::Missing => PayloadPresence::Uninspected,
        model::Presence::Absent => PayloadPresence::Absent,
        model::Presence::Present => PayloadPresence::Present,
    }
}

#[test]
fn qualify_source_bound_macro_coverage_carriers_and_children_before_collector() {
    let scope = CatalogScope::new(19);
    let mut views = 0;
    let mut malformed = 0;
    for code in [5, 6] {
        for case in observed::CASES {
            let source = observed::source(code, case);
            let image = capture(&source.bytes, scope);
            let object = scope.object_id(10);
            let raw = image
                .read_object_payloads(object, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(raw.body().presence(), PayloadPresence::Absent);
            assert_eq!(raw.module().presence(), PayloadPresence::Absent);
            assert_eq!(raw.extra().as_bytes(), Some(source.bag.as_slice()));
            let extra = image
                .inspect_table_extra(object, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(extra.object_id(), object);
            assert_eq!((extra.native_id(), extra.parent_id()), (10, 0));
            assert_eq!((extra.catalog_page(), extra.catalog_slot()), (4, 2));
            assert_eq!(extra.raw(), Some(source.bag.as_slice()));
            let inventory = extra
                .inspect_macro_inventory(&mut budget(), &|| false)
                .unwrap();
            assert!(std::ptr::eq(inventory.source(), &extra));
            assert!(
                image
                    .inspect_object_children(object, &mut budget(), &|| false)
                    .unwrap()
                    .is_empty()
            );
            match case {
                observed::Case::NamesOnly => {
                    assert_eq!(inventory.unrepresented_names(), [0, 1]);
                    assert!(inventory.properties().is_empty());
                }
                observed::Case::OpaqueExtra => {
                    assert_eq!(extra.coverage(), TableExtraCoverage::Opaque);
                    assert!(inventory.properties().is_empty());
                }
                observed::Case::OpaqueGroup => {
                    assert_eq!(inventory.opaque_groups(), [0]);
                }
                observed::Case::NonTextProperty => {
                    assert!(matches!(
                        inventory.properties()[0].observation(),
                        MacroPropertyObservation::NoText
                    ));
                }
                observed::Case::MalformedXml => {
                    assert!(matches!(
                        inventory.properties()[0].observation(),
                        MacroPropertyObservation::MalformedXml
                    ));
                }
                _ => {
                    assert!(inventory.properties().iter().all(|property| matches!(
                        property.observation(),
                        MacroPropertyObservation::Envelopes(_)
                    )));
                }
            }
            assert!(image.as_bytes() == source.bytes, "Original image changed");
            views += 1;
        }

        for case in model::CASES {
            let source = model::source(code, case);
            let image = capture(&source.bytes, scope);
            let object = scope.object_id(source.id);
            let raw = image
                .read_object_payloads(object, &mut budget(), &|| false)
                .unwrap();
            let extra = image
                .inspect_table_extra(object, &mut budget(), &|| false)
                .unwrap();
            let missing = source.missing_columns;

            for (actual, expected) in [(raw.body(), &source.body), (raw.module(), &source.module)] {
                assert_eq!(actual.as_bytes(), expected.as_deref());
                assert_eq!(
                    actual.presence(),
                    if missing {
                        PayloadPresence::Uninspected
                    } else if expected.is_some() {
                        PayloadPresence::Present
                    } else {
                        PayloadPresence::Absent
                    }
                );
            }
            assert_eq!(raw.extra().as_bytes(), source.extra.as_deref());
            assert_eq!(extra.raw(), source.extra.as_deref());
            assert_eq!(
                extra.presence(),
                if missing {
                    PayloadPresence::Uninspected
                } else if source.extra.is_some() {
                    PayloadPresence::Present
                } else {
                    PayloadPresence::Absent
                }
            );
            assert_eq!(
                extra.coverage(),
                match source.extra.as_deref() {
                    None => TableExtraCoverage::NotPresent,
                    Some([]) => TableExtraCoverage::Empty,
                    Some(_) => TableExtraCoverage::WideMr2,
                }
            );

            let children = image
                .inspect_object_children(object, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(children.len(), source.children.len());
            for (actual, expected) in children.iter().zip(&source.children) {
                let id = scope.object_id(u64::from(expected.id as u32));
                assert_eq!(actual.object().id(), id);
                assert_eq!(actual.object().parent(), Some(object));
                assert_eq!(actual.object().name().as_str(), expected.name);
                let origins = image
                    .catalog()
                    .origins()
                    .iter()
                    .filter(|origin| origin.object_id() == id)
                    .collect::<Vec<_>>();
                assert_eq!(origins.len(), 1);
                let origin = origins[0];
                assert_eq!(origin.native_id(), expected.id);
                assert_eq!(origin.parent_id(), source.id as i32);
                assert_eq!(origin.type_code(), expected.kind);
                assert_eq!(origin.flags(), expected.flags);

                let payloads = image
                    .read_object_payloads(id, &mut budget(), &|| false)
                    .unwrap();
                for (payload, bytes) in [
                    (payloads.body(), &expected.body),
                    (payloads.module(), &expected.module),
                    (payloads.extra(), &expected.extra),
                ] {
                    assert_eq!(payload.presence(), presence(expected.presence));
                    assert_eq!(payload.as_bytes(), bytes.as_deref());
                }
                if expected.kind == 12345 {
                    assert_eq!(actual.object().kind(), ObjectKind::Unknown);
                }
            }

            if matches!(case, model::Case::ChildrenAndDangling) {
                assert!(matches!(
                    image
                        .catalog()
                        .catalog()
                        .lookup("Child", None, Some(object)),
                    Err(ModelError::AmbiguousName)
                ));
                let unrelated = image.catalog().catalog().get(scope.object_id(13)).unwrap();
                assert_eq!(unrelated.parent(), Some(scope.object_id(999)));
                assert!(
                    image
                        .catalog()
                        .catalog()
                        .get(scope.object_id(999))
                        .is_none()
                );
            }

            if matches!(case, model::Case::DuplicateRootNames) {
                assert!(matches!(
                    image
                        .catalog()
                        .catalog()
                        .lookup("Items", Some(ObjectKind::Table), None),
                    Err(ModelError::AmbiguousName)
                ));
                assert_eq!(extra.native_id(), 10);
                assert_eq!(extra.catalog_slot(), 2);
            }

            assert_eq!(
                image
                    .inspect_table_extra(
                        CatalogScope::new(20).object_id(source.id),
                        &mut budget(),
                        &|| false
                    )
                    .err(),
                Some(InspectError::UnknownObject)
            );
            assert!(image.as_bytes() == source.bytes, "Original image changed");
            views += 1;
        }

        let source = guard_selector_sources::model::source(
            code,
            guard_selector_sources::model::Variant::DuplicateKind,
        );
        let image = capture(&source.bytes, scope);
        let payloads = image
            .read_object_payloads(scope.object_id(10), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(payloads.extra().presence(), PayloadPresence::Present);
        assert_eq!(payloads.extra().as_bytes(), Some(source.bag.as_slice()));
        // The actual old selector SOURCE owner already qualifies duplicate parsed
        // group kinds as malformed, even though their raw carrier is readable.
        assert_eq!(
            image
                .inspect_table_extra(scope.object_id(10), &mut budget(), &|| false)
                .err(),
            Some(InspectError::Malformed)
        );
        assert!(image.as_bytes() == source.bytes, "Original image changed");
        malformed += 1;
    }
    assert_eq!((views, malformed), (30, 2));
}

mod collector_api;
