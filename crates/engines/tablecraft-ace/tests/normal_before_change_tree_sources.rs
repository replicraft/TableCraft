//! Read-only qualification of complete authored retained-tree sources/outcomes.
#![cfg(feature = "journal")]

use std::{cmp::Ordering, io::Cursor};
use tablecraft_ace::{
    BeforeEventIntents, IndexCheckStatus, NativeImage, PayloadPresence, RuleCheckStatus,
    TableExtraCoverage, check_table, check_table_constraints, read_table, read_table_index,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::canonical_indexed_before_change as baseline;
use tablecraft_testkit::access::canonical_tree_before_change::{self as model, Case, Recipe};

const SCOPE: CatalogScope = CatalogScope::new(98);
const ORIGINAL: [[i32; 2]; 3] = [[1, 10], [2, 20], [3, 30]];

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 100_000_000)
}

fn record(page: &[u8], slot: usize) -> &[u8] {
    let at = 14 + 2 * slot;
    let start = usize::from(u16::from_le_bytes(page[at..at + 2].try_into().unwrap()));
    let end = if slot == 0 {
        4096
    } else {
        usize::from(u16::from_le_bytes(page[at - 2..at].try_into().unwrap()))
    };
    assert_eq!(start & 0xe000, 0);
    &page[start..end]
}

fn shape(case: &Case, recipe: Recipe, bytes: &[u8], after: bool) {
    let seed = match recipe {
        Recipe::NodeDescendingPrefix => baseline::Recipe::Descending,
        Recipe::NodeCompositeMacro => baseline::Recipe::Composite,
        Recipe::NodeUniqueSwap => baseline::Recipe::UniqueSwap,
        Recipe::NodeRetainedUpper => baseline::Recipe::RestoreOriginal,
        _ => baseline::Recipe::Ascending,
    };
    let original = baseline::source(
        u32::from_le_bytes(bytes[20..24].try_into().unwrap()) as u8,
        seed,
    )
    .before;
    assert_eq!(bytes.len(), if case.node { 12 * 4096 } else { 10 * 4096 });
    let maps = &bytes[3 * 4096..4 * 4096];
    for number in [0, 1, 2, 5, 6, 7] {
        assert_eq!(
            &bytes[number * 4096..(number + 1) * 4096],
            &original[number * 4096..(number + 1) * 4096],
        );
    }
    if !after {
        assert_eq!(&bytes[8 * 4096..9 * 4096], &original[8 * 4096..9 * 4096]);
    }
    assert_eq!(&maps[12..14], &17_u16.to_le_bytes());
    for slot in 0..16 {
        assert_eq!(
            record(maps, slot),
            record(&original[3 * 4096..4 * 4096], slot)
        );
    }
    let mut owned = [0; 13];
    owned[6] = if case.node { 14 } else { 2 };
    assert_eq!(record(maps, 16), owned);
    assert_eq!(record(maps, 3), &[0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0]);
    assert_eq!(record(maps, 7), record(maps, 3));
    for slot in 8..16 {
        assert_eq!(record(maps, slot), &[0; 13]);
    }
    assert_eq!(record(&bytes[4096..2 * 4096], 0), &[0; 13]);
    let width = if recipe == Recipe::NodeCompositeMacro {
        14
    } else {
        9
    };
    let seed = if recipe == Recipe::NodeAscending {
        0
    } else {
        4
    };
    for &number in &case.owned {
        let page = &bytes[number as usize * 4096..(number as usize + 1) * 4096];
        let node = case.node && number == 9;
        let prefix = if after
            && ((recipe == Recipe::LeafPrefixLoss && number == 9)
                || (recipe == Recipe::NodePrefixLoss && number == 10))
        {
            1
        } else {
            seed
        };
        let count = if node {
            if case.zero_tail { 2 } else { 1 }
        } else if !case.node {
            3
        } else if number == 10 {
            2
        } else {
            1
        };
        let full = width + if node { 4 } else { 0 };
        let active = count * full - (count - 1) * prefix;
        assert_eq!(&page[..2], &[if node { 3 } else { 4 }, 1]);
        assert_eq!(&page[2..4], &((3616 - active) as u16).to_le_bytes());
        assert_eq!(&page[4..8], &7_u32.to_le_bytes());
        assert_eq!(&page[8..12], &[0; 4]);
        assert_eq!(page[26], u8::from(node));
        assert_eq!(&page[24..26], &(prefix as u16).to_le_bytes());
        let previous = if number == 11 { 10_u32 } else { 0 };
        let next = if number == 10 { 11_u32 } else { 0 };
        let tail = if node && !case.zero_tail { 11_u32 } else { 0 };
        assert_eq!(&page[12..16], &previous.to_le_bytes());
        assert_eq!(&page[16..20], &next.to_le_bytes());
        assert_eq!(&page[20..24], &tail.to_le_bytes());
        let mut bitmap = [0; 453];
        for ordinal in 0..count {
            let end = full + ordinal * (full - prefix);
            bitmap[end / 8] |= 1 << (end % 8);
        }
        assert_eq!(&page[27..480], &bitmap);
        assert!(page[480 + active..].iter().all(|byte| *byte == 0xa7));
    }
    if case.retained_upper {
        let root = &bytes[9 * 4096..10 * 4096];
        assert_eq!(
            &root[480..493],
            &[0x7f, 0x80, 0, 0, 25, 0, 0, 8, 1, 0, 0, 0, 10]
        );
    }
}

fn qualify(case: &Case, recipe: Recipe, bytes: &[u8], values: &[[i32; 2]; 3], after: bool) {
    shape(case, recipe, bytes, after);
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap();
    let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), 3);
    assert_eq!(checked.indexes().len(), 1);
    let selected = &checked.indexes()[0];
    assert_eq!(selected.status(), IndexCheckStatus::Checked);
    let index = selected.index();
    assert_eq!(
        (index.data_number(), index.root_page(), index.usage_map()),
        (0, 9, (3 << 8) | 16)
    );
    assert_eq!(
        index
            .columns()
            .iter()
            .map(|column| (column.number(), column.descending()))
            .collect::<Vec<_>>(),
        case.components
    );
    assert_eq!(index.constraints().unwrap().unique(), case.unique);
    assert!(!index.constraints().unwrap().ignore_nulls());
    let expected = values
        .iter()
        .map(|row| row.iter().copied().map(Value::Int32).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let table = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(table.rows(), expected);
    assert_eq!(table.schema().object_id(), SCOPE.object_id(7));
    for field in table.schema().columns() {
        assert_eq!(
            (
                field.native_type(),
                field.flags(),
                field.extended_flags(),
                field.storage_length()
            ),
            (4, 3, 0, 4)
        );
    }
    let indexed = read_table_index(
        &mut image.reader(),
        SCOPE,
        "Items",
        index.name(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut ordered = expected;
    ordered.sort_by(|left, right| {
        for &(column, descending) in &case.components {
            let (Value::Int32(left), Value::Int32(right)) = (&left[column], &right[column]) else {
                panic!("literal I32 source");
            };
            let ordering = if descending {
                right.cmp(left)
            } else {
                left.cmp(right)
            };
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
        Ordering::Equal
    });
    assert_eq!(indexed.rows(), ordered);
    let constraints =
        check_table_constraints(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(constraints.rows(), 3);
    assert_eq!(constraints.table_validation(), RuleCheckStatus::NotDeclared);
    assert!(constraints.relationships().is_empty());
    assert!(
        constraints
            .fields()
            .iter()
            .all(|field| field.required() == RuleCheckStatus::NotDeclared
                && field.empty_text() == RuleCheckStatus::NotDeclared
                && field.validation() == RuleCheckStatus::NotDeclared)
    );
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
    assert!(bound.final_rows().unwrap().is_empty());
    assert!(bound.rejection().is_none());
    assert_eq!(image.as_bytes(), bytes);
}

#[test]
fn qualify_retained_tree_originals_and_fitting_expectations_before_normal_writer() {
    let mut views = 0;
    let mut bytes = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            qualify(&case, recipe, &case.before, &ORIGINAL, false);
            qualify(&case, recipe, &case.expected, &case.final_rows, true);
            assert_eq!(&case.expected[..8 * 4096], &case.before[..8 * 4096]);
            if case.retained_upper {
                assert_eq!(case.before, case.expected);
                assert_eq!(case.changed_rows, 0);
            }
            views += 2;
            bytes += case.before.len() + case.expected.len();
        }
    }
    assert_eq!((views, bytes), (36, 1_703_936));
}

#[path = "normal_before_change_tree_sources/export.rs"]
mod export;

#[path = "normal_before_change_tree_sources/anchors.rs"]
mod anchors;
