//! Read-only qualification before the Standard fixed ownership guard changes.
use super::model::{self, Case, Family, PAGE};
use std::io::Cursor;
use tablecraft_ace::{
    IndexCheckStatus, NativeImage, RuleCheckStatus, check_table, check_table_constraints,
    inspect_table_indexes, read_table,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(1);

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 100_000_000)
}

fn selected(bytes: &[u8], after: bool) {
    let table = read_table(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        table.rows(),
        &[
            vec![Value::Int32(1), Value::Int32(if after { 11 } else { 10 })],
            vec![Value::Int32(2), Value::Int32(20)],
            vec![Value::Int32(3), Value::Int32(30)],
        ]
    );
    let checked = check_table(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 3);
    assert_eq!(checked.indexes().len(), 2);
    for index in checked.indexes() {
        assert_eq!(index.status(), IndexCheckStatus::Checked);
        assert_eq!(index.index().data_number(), 0);
        assert_eq!(index.index().root_page(), 13);
        assert_eq!(index.index().usage_map(), 14 << 8);
    }
    let report = check_table_constraints(
        &mut Cursor::new(bytes),
        SCOPE,
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(report.rows(), 3);
    assert_eq!(report.fields().len(), 2);
    assert_eq!(report.table_validation(), RuleCheckStatus::NotDeclared);
    assert!(report.relationships().is_empty());
    for field in report.fields() {
        assert_eq!(field.required(), RuleCheckStatus::NotDeclared);
        assert_eq!(field.validation(), RuleCheckStatus::NotDeclared);
    }
}

fn role(bytes: &[u8], case: &Case, overlapping: bool) {
    let start = model::row_start(bytes, case.claimed_map, 0);
    assert_eq!(bytes[start], 0);
    assert_eq!(&bytes[start + 1..start + 5], &[0; 4]);
    assert_eq!(
        (bytes[start + 5 + 12 / 8] >> (12 % 8)) & 1,
        u8::from(overlapping)
    );
    let root = if case.family == Family::SameTable {
        13
    } else {
        model::FOREIGN_ROOT
    };
    assert_eq!((bytes[start + 5 + root / 8] >> (root % 8)) & 1, 1);
    assert_eq!(&bytes[12 * PAGE..12 * PAGE + 2], &[1, 1]);
    assert_eq!(&bytes[12 * PAGE + 4..12 * PAGE + 8], &10_u32.to_le_bytes());

    if let Some(physical) = case.physical_start {
        assert_eq!(&bytes[7 * PAGE + 51..7 * PAGE + 55], &1_u32.to_le_bytes());
        assert_eq!(
            &bytes[7 * PAGE + physical + 34..7 * PAGE + physical + 38],
            &((model::FOREIGN_MAP as u32) << 8).to_le_bytes()
        );
        assert_eq!(
            &bytes[7 * PAGE + physical + 38..7 * PAGE + physical + 42],
            &(model::FOREIGN_ROOT as u32).to_le_bytes()
        );
        assert_eq!(
            &bytes[model::FOREIGN_ROOT * PAGE..model::FOREIGN_ROOT * PAGE + 2],
            &[4, 1]
        );
        assert_eq!(
            &bytes[model::FOREIGN_ROOT * PAGE + 4..model::FOREIGN_ROOT * PAGE + 8],
            &7_u32.to_le_bytes()
        );
        let inspection = inspect_table_indexes(
            &mut Cursor::new(bytes),
            SCOPE,
            "MSysRelationships",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let count = usize::from(case.family == Family::Foreign);
        assert_eq!(inspection.indexes().len(), count);
        for index in inspection.indexes() {
            assert_eq!(index.name(), "RelationKey");
            assert_eq!(index.data_number(), 0);
            assert_eq!(index.root_page() as usize, model::FOREIGN_ROOT);
            assert_eq!(index.usage_map(), (model::FOREIGN_MAP as u32) << 8);
        }
        let checked = check_table(
            &mut Cursor::new(bytes),
            SCOPE,
            "MSysRelationships",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(checked.rows(), 0);
        assert_eq!(checked.indexes().len(), count);
        assert!(
            checked
                .indexes()
                .iter()
                .all(|index| index.status() == IndexCheckStatus::Checked)
        );
        // The orphan case intentionally has no logical index projection. Its
        // physical descriptor and source-owned map remain explicit raw facts.
        assert_eq!(
            &bytes[7 * PAGE + 47..7 * PAGE + 51],
            &(count as u32).to_le_bytes()
        );
    }
}

#[test]
fn qualify_fixed_index_body_same_foreign_and_orphan_sources_before_guard() {
    let mut pairs = 0;
    let mut negatives = 0;
    for code in [1, 5, 6] {
        for family in [Family::SameTable, Family::Foreign, Family::Orphan] {
            let case = model::case(code, family);
            for (bytes, after, overlapping) in [
                (&case.before, false, false),
                (&case.expected, true, false),
                (&case.bad, false, true),
            ] {
                let image =
                    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false)
                        .unwrap();
                assert_eq!(image.as_bytes(), bytes);
                selected(bytes, after);
                role(bytes, &case, overlapping);
            }
            let changes = case
                .before
                .iter()
                .zip(&case.expected)
                .enumerate()
                .filter_map(|(offset, (a, b))| (a != b).then_some(offset))
                .collect::<Vec<_>>();
            assert_eq!(changes, [model::PATCH, model::KEY_PATCH]);
            let start = model::row_start(&case.before, case.claimed_map, 0);
            let changes = case
                .before
                .iter()
                .zip(&case.bad)
                .enumerate()
                .filter_map(|(offset, (a, b))| (a != b).then_some((offset, a ^ b)))
                .collect::<Vec<_>>();
            assert_eq!(changes, [(start + 5 + 12 / 8, 1 << (12 % 8))]);
            pairs += 1;
            negatives += 1;
        }
    }
    assert_eq!((pairs, negatives), (9, 9));
}
