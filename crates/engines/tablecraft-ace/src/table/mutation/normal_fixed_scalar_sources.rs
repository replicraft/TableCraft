//! Read-only source admission; no event evaluation, normal plan or persistence.
use crate::{
    NativeImage, PayloadPresence, RuleCheckStatus, TableExtraCoverage, check_table,
    check_table_constraints, read_table,
    table::mutation::{guards::native_events, preserved_source},
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::canonical_fixed_before_change::{self as model, Case};

const SCOPE: CatalogScope = CatalogScope::new(110);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn qualify(case: &Case, bytes: &[u8], rows: &[Vec<Value>]) {
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap();
    let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), 2);
    assert_eq!(checked.schema().columns().len(), 10);
    assert!(checked.indexes().is_empty());
    for (native, name, offset, width) in model::FIELDS {
        let field = &checked.schema().columns()[native];
        assert_eq!(field.name(), name);
        assert_eq!(field.number(), native);
        assert_eq!(field.flags(), 3);
        assert_eq!(field.extended_flags(), 0);
        assert!(field.fixed());
        assert_eq!(field.storage_length(), width);
        // Offset facts are also witnessed by exact original raw spans below;
        // no production descriptor/record encoder supplies a golden.
        let expected_type = match native {
            0 => 1,
            3 => 4,
            4 => 19,
            5 => 5,
            8 => 8,
            _ => unreachable!(),
        };
        assert_eq!(field.native_type(), expected_type);
        assert!(offset + width <= 59);
    }
    let read = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(read.rows(), rows);
    let constraints =
        check_table_constraints(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(constraints.rows(), 2);
    assert!(constraints.relationships().is_empty());
    assert_eq!(constraints.table_validation(), RuleCheckStatus::NotDeclared);
    let coverage = image
        .inspect_table_macro_coverage(SCOPE.object_id(7), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(coverage.body().presence(), PayloadPresence::Absent);
    assert_eq!(coverage.module().presence(), PayloadPresence::Absent);
    assert!(coverage.children().is_empty());
    assert_eq!(
        coverage.table_extra().coverage(),
        TableExtraCoverage::WideMr2
    );
    assert_eq!(coverage.table_extra().raw(), Some(case.extra.as_slice()));
    let group = &coverage.table_extra().groups()[0];
    assert_eq!(group.kind(), 3);
    let properties = group.properties().unwrap();
    assert_eq!(properties.len(), 1);
    assert_eq!(properties[0].name(), "StoredGuard");
    properties[0]
        .inspect_complete_before_events(&mut budget(), &|| false)
        .unwrap();
    // Bind the actual native-owned property references and capture the real
    // preserved Source. Neither operation evaluates an event row.
    let selected =
        native_events::programs(&image, checked.schema(), &mut budget(), &|| false).unwrap();
    let source = preserved_source::capture_native_before_change(
        &image,
        "Items",
        &selected,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(source.table.rows(), rows);
    assert_eq!(source.records.len(), 2);
    assert!(source.dependencies.is_empty());
    assert_eq!(image.as_bytes(), bytes);
}

fn complement(case: &Case, expected: &[u8]) {
    let mut allowed = vec![false; case.before.len()];
    for group in &case.groups {
        let (_, _, offset, width) = model::FIELDS
            .iter()
            .find(|field| field.0 == group.column)
            .unwrap();
        for update in &group.updates {
            let (start, end) = model::record(&case.before, update.ordinal);
            allowed[start + 2 + offset..start + 2 + offset + width].fill(true);
            allowed[end - 2 + group.column / 8] = true;
        }
    }
    for &ordinal in &case.event_rows {
        let (start, end) = model::record(&case.before, ordinal);
        allowed[start + 5..start + 9].fill(true);
        // The macro changes only a present I32 payload; its mask stays exact.
        assert_eq!(case.before[end - 2] & 8, expected[end - 2] & 8);
    }
    for (at, (before, after)) in case.before.iter().zip(expected).enumerate() {
        if !allowed[at] {
            assert_eq!(before, after, "unrelated byte {at}");
        }
    }
    for ordinal in 0..2 {
        let (_, end) = model::record(&case.before, ordinal);
        let mut bits = [0_u8; 2];
        for group in &case.groups {
            if group.updates.iter().any(|update| update.ordinal == ordinal) {
                bits[group.column / 8] |= 1 << (group.column % 8);
            }
        }
        for (byte, changed) in bits.into_iter().enumerate() {
            assert_eq!(
                case.before[end - 2 + byte] & !changed,
                expected[end - 2 + byte] & !changed
            );
        }
    }
}

#[test]
fn qualify_fixed_scalar_before_change_sources_before_normal_typed_producer() {
    let mut originals = 0;
    let mut expectations = 0;
    let mut roots = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            assert_eq!(case.before.len(), 9 * 4096);
            qualify(&case, &case.before, &case.rows);
            originals += 1;
            if let Some(expected) = &case.expected {
                complement(&case, expected);
                qualify(&case, expected, case.final_rows.as_ref().unwrap());
                expectations += 1;
                roots += case.changed_rows;
            } else {
                assert!(case.refusal.is_some());
                assert!(case.final_rows.is_none());
            }
        }
    }
    assert_eq!((originals, expectations, roots), (38, 28, 28));
}

mod export;

#[path = "normal_fixed_scalar_sources/anchors.rs"]
mod anchors;
