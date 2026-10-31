//! Source-bound observation only; no absence or normal-save permission.
use super::{budget, capture, guard_selector_sources, model, observed, presence};
use crate::{InspectError, NativeImage, PayloadPresence, TableMacroCoverage};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

fn assert_source(report: &TableMacroCoverage<'_>, image: &NativeImage, native_id: u64) {
    let object = image.catalog().catalog().scope().object_id(native_id);
    assert!(std::ptr::eq(report.image(), image));
    let original = image
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.object_id() == object)
        .unwrap();
    assert!(std::ptr::eq(report.table_origin(), original));
    assert_eq!(report.table_extra().object_id(), object);
    assert_eq!(report.table_extra().native_id(), native_id as i32);
    assert_eq!(report.table_extra().parent_id(), original.parent_id());
    assert_eq!(report.table_extra().catalog_page(), original.page());
    assert_eq!(report.table_extra().catalog_slot(), original.slot());
}

#[test]
fn macro_coverage_retains_all_qualified_original_carriers_and_children() {
    let scope = CatalogScope::new(19);
    let mut reports = 0;
    let mut malformed = 0;
    for code in [5, 6] {
        for case in observed::CASES {
            let source = observed::source(code, case);
            let image = capture(&source.bytes, scope);
            let report = image
                .inspect_table_macro_coverage(scope.object_id(10), &mut budget(), &|| false)
                .unwrap();
            assert_source(&report, &image, 10);
            assert_eq!(report.body().presence(), PayloadPresence::Absent);
            assert_eq!(report.module().presence(), PayloadPresence::Absent);
            assert_eq!(report.table_extra().raw(), Some(source.bag.as_slice()));
            assert!(report.children().is_empty());
            assert!(image.as_bytes() == source.bytes);
            reports += 1;
        }

        for case in model::CASES {
            let source = model::source(code, case);
            let image = capture(&source.bytes, scope);
            let report = image
                .inspect_table_macro_coverage(scope.object_id(source.id), &mut budget(), &|| false)
                .unwrap();
            assert_source(&report, &image, source.id);
            for (payload, expected) in [
                (report.body(), &source.body),
                (report.module(), &source.module),
            ] {
                assert_eq!(payload.as_bytes(), expected.as_deref());
                assert_eq!(
                    payload.presence(),
                    if source.missing_columns {
                        PayloadPresence::Uninspected
                    } else if expected.is_some() {
                        PayloadPresence::Present
                    } else {
                        PayloadPresence::Absent
                    }
                );
            }
            assert_eq!(report.table_extra().raw(), source.extra.as_deref());
            assert_eq!(
                report.table_extra().presence(),
                if source.missing_columns {
                    PayloadPresence::Uninspected
                } else if source.extra.is_some() {
                    PayloadPresence::Present
                } else {
                    PayloadPresence::Absent
                }
            );
            assert_eq!(report.children().len(), source.children.len());
            for expected in &source.children {
                let id = scope.object_id(u64::from(expected.id as u32));
                let child = report
                    .children()
                    .iter()
                    .find(|child| child.object_id() == id)
                    .unwrap();
                let descriptor = image.catalog().catalog().get(id).unwrap();
                let origin = image
                    .catalog()
                    .origins()
                    .iter()
                    .find(|origin| origin.object_id() == id)
                    .unwrap();
                assert!(std::ptr::eq(child.descriptor(), descriptor));
                assert!(std::ptr::eq(child.origin(), origin));
                assert_eq!(
                    child.descriptor().parent(),
                    Some(report.table_origin().object_id())
                );
                assert_eq!(child.descriptor().name().as_str(), expected.name);
                assert_eq!(child.origin().native_id(), expected.id);
                assert_eq!(child.origin().parent_id(), source.id as i32);
                assert_eq!(child.origin().type_code(), expected.kind);
                assert_eq!(child.origin().flags(), expected.flags);
                assert_eq!(child.payloads().object_id(), id);

                for (payload, raw) in [
                    (child.payloads().body(), &expected.body),
                    (child.payloads().module(), &expected.module),
                    (child.payloads().extra(), &expected.extra),
                ] {
                    assert_eq!(payload.presence(), presence(expected.presence));
                    assert_eq!(payload.as_bytes(), raw.as_deref());
                }
            }
            assert!(
                !report
                    .children()
                    .iter()
                    .any(|child| child.object_id() == scope.object_id(13))
            );
            assert!(image.as_bytes() == source.bytes);
            reports += 1;
        }

        let source = guard_selector_sources::model::source(
            code,
            guard_selector_sources::model::Variant::DuplicateKind,
        );
        let image = capture(&source.bytes, scope);
        assert_eq!(
            image
                .inspect_table_macro_coverage(scope.object_id(10), &mut budget(), &|| false)
                .err(),
            Some(InspectError::Malformed)
        );
        assert!(image.as_bytes() == source.bytes);
        malformed += 1;
    }
    assert_eq!((reports, malformed), (30, 2));
}

#[test]
fn macro_coverage_keeps_image_scope_and_rejects_unknown_or_non_table_roots() {
    let first = model::source(5, model::Case::NullExtra);
    let second = model::source(6, model::Case::EmptyExtra);
    let first_scope = CatalogScope::new(19);
    let second_scope = CatalogScope::new(20);
    let a = capture(&first.bytes, first_scope);
    let b = capture(&second.bytes, second_scope);
    let a_report = a
        .inspect_table_macro_coverage(first_scope.object_id(10), &mut budget(), &|| false)
        .unwrap();
    let b_report = b
        .inspect_table_macro_coverage(second_scope.object_id(10), &mut budget(), &|| false)
        .unwrap();
    assert_source(&a_report, &a, 10);
    assert_source(&b_report, &b, 10);
    assert!(!std::ptr::eq(a_report.image(), b_report.image()));
    assert_eq!(a_report.table_extra().presence(), PayloadPresence::Absent);
    assert_eq!(b_report.table_extra().presence(), PayloadPresence::Present);
    assert_eq!(b_report.table_extra().raw(), Some([].as_slice()));
    assert_eq!(
        b.inspect_table_macro_coverage(first_scope.object_id(10), &mut budget(), &|| false)
            .err(),
        Some(InspectError::UnknownObject)
    );
    assert_eq!(
        a.inspect_table_macro_coverage(first_scope.object_id(999), &mut budget(), &|| false)
            .err(),
        Some(InspectError::UnknownObject)
    );

    let source = model::source(6, model::Case::ChildrenAndDangling);
    let image = capture(&source.bytes, first_scope);
    assert_eq!(
        image
            .inspect_table_macro_coverage(first_scope.object_id(11), &mut budget(), &|| false)
            .err(),
        Some(InspectError::Unsupported)
    );
}

fn collect(
    bytes: &[u8],
    allowance: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    let scope = CatalogScope::new(19);
    let image = NativeImage::capture(&mut Cursor::new(bytes), scope, allowance, cancelled)?;
    let report = image.inspect_table_macro_coverage(scope.object_id(10), allowance, cancelled)?;
    assert_source(&report, &image, 10);
    assert_eq!(report.children().len(), 2);
    Ok(())
}

#[test]
fn macro_coverage_capture_and_collection_exact_pools_and_final_cancel_are_typed() {
    let source = model::source(6, model::Case::ChildrenAndDangling);
    let mut measured = budget();
    let polls = Cell::new(0);
    collect(&source.bytes, &mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    let resources = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let used =
        resources.map(|resource| budget().remaining(resource) - measured.remaining(resource));
    assert!(used.iter().all(|amount| *amount > 0));
    collect(
        &source.bytes,
        &mut ResourceBudget::new(used[0], used[1], used[2]),
        &|| false,
    )
    .unwrap();
    for position in 0..resources.len() {
        let mut limits = used;
        limits[position] -= 1;
        assert!(matches!(
            collect(
                &source.bytes,
                &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
                &|| false
            ),
            Err(InspectError::Limit(_))
        ));
    }

    let last = polls.get();
    assert!(last > 0);
    polls.set(0);
    assert_eq!(
        collect(&source.bytes, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == last
        })
        .err(),
        Some(InspectError::Cancelled)
    );
    assert_eq!(polls.get(), last);
    assert_eq!(
        collect(&source.bytes, &mut budget(), &|| true).err(),
        Some(InspectError::Cancelled)
    );
}
