//! SOURCE-only integration gate. Never calls a native planner, preview, stage,
//! journal preparation, commit or recovery function.
#![cfg(feature = "journal")]

use std::io::Cursor;
use tablecraft_ace::{
    BeforeEventIntents, IndexCheckStatus, NativeImage, PayloadPresence, RuleCheckStatus,
    TableExtraCoverage, check_table, check_table_constraints, read_table, read_table_index,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_before_change,
    canonical_indexed_before_change::{self as model, Case, Recipe},
};

const SCOPE: CatalogScope = CatalogScope::new(96);
const ORIGINAL: [[i32; 2]; 3] = [[1, 10], [2, 20], [3, 30]];

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 100_000_000)
}

fn record(page: &[u8], slot: usize) -> &[u8] {
    let word = 14 + 2 * slot;
    let start = usize::from(u16::from_le_bytes(page[word..word + 2].try_into().unwrap()));
    let end = if slot == 0 {
        4096
    } else {
        usize::from(u16::from_le_bytes(page[word - 2..word].try_into().unwrap()))
    };
    assert_eq!(start & 0xe000, 0);
    &page[start..end]
}

fn raw_roles(case: &Case, header: u8, bytes: &[u8]) {
    assert_eq!(bytes.len(), 10 * 4096);
    let seed = canonical_before_change::source(header, canonical_before_change::Recipe::Sequential);
    for page in [0, 1, 2, 5, 6] {
        assert_eq!(
            &bytes[page * 4096..(page + 1) * 4096],
            &seed.before[page * 4096..(page + 1) * 4096]
        );
    }
    let maps = &bytes[3 * 4096..4 * 4096];
    assert_eq!(&maps[12..14], &17_u16.to_le_bytes());
    assert_eq!(&maps[4..8], &[0; 4]);
    for slot in 0..16 {
        assert_eq!(
            record(maps, slot),
            record(&seed.before[3 * 4096..4 * 4096], slot)
        );
    }
    let mut owned = [0; 13];
    owned[6] = 2;
    assert_eq!(record(maps, 16), owned);
    // Existing DATA8 roles are not widened to include INDEX9.
    assert_eq!(record(maps, 3), &[0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0]);
    assert_eq!(record(maps, 7), record(maps, 3));
    let global = record(&bytes[4096..2 * 4096], 0);
    assert_eq!(global, &[0; 13]);
    let td = &bytes[7 * 4096..8 * 4096];
    assert_eq!(
        &td[47..51],
        &(if case.alias { 2_u32 } else { 1_u32 }).to_le_bytes()
    );
    assert_eq!(&td[51..55], &1_u32.to_le_bytes());
    assert_eq!(&td[55..59], &((3_u32 << 8) | 3).to_le_bytes());
    assert_eq!(&td[59..63], &((3_u32 << 8) | 7).to_le_bytes());
    assert_eq!(&td[67..71], &3_u32.to_le_bytes());
    assert_eq!(&td[143..147], &1923_u32.to_le_bytes());
    assert_eq!(&td[177..181], &((3_u32 << 8) | 16).to_le_bytes());
    assert_eq!(&td[181..185], &9_u32.to_le_bytes());
    let index = &bytes[9 * 4096..10 * 4096];
    assert_eq!(&index[..2], &[4, 1]);
    assert_eq!(&index[4..8], &7_u32.to_le_bytes());
    assert_eq!(&index[8..27], &[0; 19]);
    let active = 3 * (5 * case.components.len() + 4);
    assert_eq!(&index[2..4], &((3616 - active) as u16).to_le_bytes());
    assert!(index[480 + active..].iter().all(|byte| *byte == 0xa7));
}

fn qualify(case: &Case, header: u8, bytes: &[u8], values: &[[i32; 2]; 3]) {
    raw_roles(case, header, bytes);
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap();
    let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), 3);
    assert_eq!(checked.schema().definition_page(), 7);
    assert_eq!(checked.indexes().len(), if case.alias { 2 } else { 1 });
    let expected = values
        .iter()
        .map(|row| row.iter().copied().map(Value::Int32).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let rows = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(rows.rows(), expected);
    for (column, name) in checked.schema().columns().iter().zip(["Id", "Score"]) {
        assert_eq!(column.name(), name);
        assert_eq!(
            (
                column.native_type(),
                column.flags(),
                column.extended_flags(),
                column.storage_length()
            ),
            (4, 3, 0, 4)
        );
        assert!(column.fixed());
    }
    for index in checked.indexes() {
        assert_eq!(index.status(), IndexCheckStatus::Checked);
        let index = index.index();
        assert_eq!(
            (index.data_number(), index.root_page(), index.usage_map()),
            (0, 9, (3 << 8) | 16)
        );
        assert_eq!(
            index
                .columns()
                .iter()
                .map(|key| (key.number(), key.descending()))
                .collect::<Vec<_>>(),
            case.components
        );
        let constraints = index.constraints().unwrap();
        assert_eq!(constraints.unique(), case.unique);
        assert!(!constraints.primary_key());
        assert!(!constraints.ignore_nulls());
        let indexed = read_table_index(
            &mut image.reader(),
            SCOPE,
            "Items",
            index.name(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let mut sorted = expected.clone();
        sorted.sort_by(|left, right| {
            for &(column, descending) in &case.components {
                let (Value::Int32(left), Value::Int32(right)) = (&left[column], &right[column])
                else {
                    panic!("literal I32 rows");
                };
                let order = if descending {
                    right.cmp(left)
                } else {
                    left.cmp(right)
                };
                if !order.is_eq() {
                    return order;
                }
            }
            std::cmp::Ordering::Equal
        });
        assert_eq!(indexed.rows(), sorted);
    }
    let constraints =
        check_table_constraints(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(constraints.rows(), 3);
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
    assert_eq!(
        (
            coverage.table_extra().native_id(),
            coverage.table_extra().parent_id()
        ),
        (7, 0x0f00_0001)
    );
    assert_eq!(
        (
            coverage.table_extra().catalog_page(),
            coverage.table_extra().catalog_slot()
        ),
        (4, 7)
    );
    // Read-only discovery/complete compile with no supplied row: no event is
    // evaluated and no physical plan or candidate can be obtained here.
    let bound = image
        .prepare_native_before_change_fields(
            SCOPE.object_id(7),
            BeforeEventIntents::FixedUpdate {
                column: "Score",
                updates: &[],
            },
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(bound.source_bytes(), bytes);
    assert!(bound.final_rows().unwrap().is_empty());
    assert!(bound.rejection().is_none());
    assert_eq!(image.as_bytes(), bytes);
}

#[test]
fn qualify_indexed_before_change_originals_and_literal_expectations_before_writer() {
    let mut originals = 0;
    let mut expectations = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            qualify(&case, header, &case.before, &ORIGINAL);
            originals += 1;
            if recipe == Recipe::UniqueConflict {
                assert!(case.expected.is_none());
                assert_eq!(case.final_rows, [[1, 20], [2, 20], [3, 30]]);
                continue;
            }
            let after = case
                .expected
                .as_ref()
                .expect("positive literal complete image");
            assert_eq!(&after[..8 * 4096], &case.before[..8 * 4096]);
            qualify(&case, header, after, &case.final_rows);
            if matches!(recipe, Recipe::RestoreOriginal | Recipe::Empty) {
                assert_eq!(after, &case.before);
                assert_eq!(case.changed_rows, 0);
            }
            expectations += 1;
        }
    }
    assert_eq!((originals, expectations), (18, 16));
}

#[path = "normal_before_change_indexed_sources/export.rs"]
mod export;

#[path = "normal_before_change_indexed_sources/anchors.rs"]
mod anchors;
