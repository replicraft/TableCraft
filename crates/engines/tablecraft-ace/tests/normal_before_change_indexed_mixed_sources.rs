//! Existing read services qualify complete mixed INDEX sources before writers.
#![cfg(feature = "journal")]

use std::{cmp::Ordering, io::Cursor};
use tablecraft_ace::{
    IndexCheckStatus, MacroProgramProfile, NativeImage, PayloadPresence, RuleCheckStatus,
    TableExtraCoverage, check_table, check_table_constraints, read_table, read_table_index,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::{
    canonical_indexed_mixed_before_change::{self as model, Case, Recipe},
    canonical_mixed_before_change as original,
};

const SCOPE: CatalogScope = CatalogScope::new(105);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
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
    let header = u32::from_le_bytes(bytes[20..24].try_into().unwrap()) as u8;
    let seed = original::source(header, recipe.seed());
    assert_eq!(
        bytes.len(),
        if case.node {
            13 * 4096
        } else if case.primary {
            11 * 4096
        } else {
            10 * 4096
        }
    );
    for number in [0, 1, 2, 4, 5, 6] {
        assert_eq!(
            &bytes[number * 4096..(number + 1) * 4096],
            &seed.before[number * 4096..(number + 1) * 4096]
        );
    }
    if !after {
        assert_eq!(&bytes[8 * 4096..9 * 4096], &seed.before[8 * 4096..9 * 4096]);
    }
    if case.primary {
        assert_eq!(
            &bytes[9 * 4096..10 * 4096],
            &seed.before[9 * 4096..10 * 4096]
        );
    }
    let maps = &bytes[3 * 4096..4 * 4096];
    let old_maps = &seed.before[3 * 4096..4 * 4096];
    let count = if case.primary { 18 } else { 17 };
    assert_eq!(&maps[12..14], &(count as u16).to_le_bytes());
    for slot in 0..count - 1 {
        assert_eq!(record(maps, slot), record(old_maps, slot));
    }
    let mut membership = [0; 13];
    for &number in &case.owned {
        membership[5 + number as usize / 8] |= 1 << (number % 8);
    }
    assert_eq!(record(maps, count - 1), membership);
    assert_eq!(record(maps, 3), &[0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0]);
    assert_eq!(record(maps, 7), record(maps, 3));
    assert_eq!(record(&bytes[4096..2 * 4096], 0), &[0; 13]);
    let root = case.root as usize;
    for &number in &case.owned {
        let number = number as usize;
        let node = case.node && number == root;
        let page = &bytes[number * 4096..(number + 1) * 4096];
        let count = if node || (case.node && number == root + 2) {
            1
        } else if case.node {
            2
        } else {
            3
        };
        let prefix = if after
            && ((recipe == Recipe::CounterLeafPrefixLoss && number == root)
                || (recipe == Recipe::CounterTreePrefixLoss && number == root + 1))
        {
            1
        } else {
            3
        };
        let width = 5 * case.components.len() + 4 + if node { 4 } else { 0 };
        let active = count * width - (count - 1) * prefix;
        assert_eq!(&page[..2], &[if node { 3 } else { 4 }, 1]);
        assert_eq!(&page[2..4], &((3616 - active) as u16).to_le_bytes());
        assert_eq!(&page[4..8], &7_u32.to_le_bytes());
        assert_eq!(&page[8..12], &[0; 4]);
        assert_eq!(&page[24..26], &(prefix as u16).to_le_bytes());
        assert_eq!(page[26], u8::from(node));
        let links = if node {
            [0, 0, root as u32 + 2]
        } else if !case.node {
            [0, 0, 0]
        } else if number == root + 1 {
            [0, root as u32 + 2, 0]
        } else {
            [root as u32 + 1, 0, 0]
        };
        for (position, value) in links.iter().enumerate() {
            assert_eq!(
                &page[12 + 4 * position..16 + 4 * position],
                &value.to_le_bytes()
            );
        }
        let mut bitmap = [0; 453];
        let mut end = 0;
        for entry in 0..count {
            end += width - if entry == 0 { 0 } else { prefix };
            bitmap[end / 8] |= 1 << (end % 8);
        }
        assert_eq!(&page[27..480], &bitmap);
        if !after {
            assert!(page[480 + active..].iter().all(|byte| *byte == 0xa7));
        } else {
            assert_eq!(
                &page[480 + active..],
                &case.before[number * 4096 + 480 + active..(number + 1) * 4096]
            );
        }
    }
    if case.retained_upper {
        let page = &bytes[root * 4096..(root + 1) * 4096];
        assert_eq!(&page[480..489], &[0x7f, 0x80, 0, 9, 0xc4, 0, 0, 8, 1]);
    }
}

fn ordered(rows: &[Vec<Value>], components: &[(usize, bool)]) -> Vec<Vec<Value>> {
    let mut indexed = rows.iter().enumerate().collect::<Vec<_>>();
    indexed.sort_by(|(left_ordinal, left), (right_ordinal, right)| {
        for &(number, descending) in components {
            let (Value::Int32(a), Value::Int32(b)) = (&left[number], &right[number]) else {
                panic!("literal I32 key");
            };
            let compare = if descending { b.cmp(a) } else { a.cmp(b) };
            if compare != Ordering::Equal {
                return compare;
            }
        }
        left_ordinal.cmp(right_ordinal)
    });
    indexed.into_iter().map(|(_, row)| row.clone()).collect()
}

fn qualify(case: &Case, bytes: &[u8], rows: &[Vec<Value>]) {
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap();
    let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), 3);
    assert_eq!(checked.schema().columns().len(), 6);
    assert_eq!(checked.indexes().len(), if case.primary { 2 } else { 1 });
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let id = &checked.schema().columns()[0];
    assert_eq!(
        (
            id.number(),
            id.native_type(),
            id.flags(),
            id.storage_length()
        ),
        (0, 4, if case.primary { 7 } else { 3 }, 4)
    );
    let score = &checked.schema().columns()[5];
    assert_eq!(
        (
            score.number(),
            score.native_type(),
            score.flags(),
            score.extended_flags(),
            score.storage_length()
        ),
        (5, 4, 3, 0, 4)
    );
    assert!(score.fixed());
    let sequential = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(sequential.rows(), rows);
    let selected = checked
        .indexes()
        .iter()
        .map(|index| index.index())
        .find(|index| index.name() == case.index_name)
        .unwrap();
    assert_eq!(
        (
            selected.number(),
            selected.data_number(),
            selected.root_page(),
            selected.usage_map()
        ),
        (
            case.data_number,
            case.data_number,
            case.root,
            case.usage_map
        )
    );
    assert_eq!(selected.flags(), if case.unique { 0x81 } else { 0x80 });
    assert_eq!(selected.constraints().unwrap().unique(), case.unique);
    assert!(!selected.constraints().unwrap().primary_key());
    assert_eq!(
        selected
            .columns()
            .iter()
            .map(|component| (component.number(), component.descending()))
            .collect::<Vec<_>>(),
        case.components
    );
    let indexed = read_table_index(
        &mut image.reader(),
        SCOPE,
        "Items",
        case.index_name,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(indexed.rows(), ordered(rows, &case.components));
    if case.primary {
        let primary = checked.indexes()[0].index();
        assert_eq!(
            (
                primary.name(),
                primary.data_number(),
                primary.root_page(),
                primary.usage_map(),
                primary.flags()
            ),
            ("PrimaryId", 0, 9, (3 << 8) | 16, 0x89)
        );
        assert!(primary.constraints().unwrap().primary_key());
        let primary_rows = read_table_index(
            &mut image.reader(),
            SCOPE,
            "Items",
            "PrimaryId",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(primary_rows.rows(), rows);
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
    let groups = coverage.table_extra().groups();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].kind(), 3);
    let properties = groups[0].properties().unwrap();
    assert_eq!(properties.len(), 1);
    assert_eq!(
        (
            properties[0].name(),
            properties[0].flags(),
            properties[0].native_type()
        ),
        ("StoredGuard", 1, 12)
    );
    properties[0]
        .inspect_macro_program(
            MacroProgramProfile::BeforeChangeFields,
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(image.as_bytes(), bytes);
}

fn complement(case: &Case, after: &[u8]) {
    assert_eq!(after.len(), case.before.len());
    let mut allowed = vec![false; after.len()];
    for ordinal in 0..3 {
        let word = 8 * 4096 + 14 + 2 * ordinal;
        let start = 8 * 4096
            + usize::from(u16::from_le_bytes(
                case.before[word..word + 2].try_into().unwrap(),
            ));
        allowed[start + 6..start + 10].fill(true);
    }
    for &number in &case.owned {
        allowed[number as usize * 4096..(number as usize + 1) * 4096].fill(true);
    }
    for ((before, after), allowed) in case.before.iter().zip(after).zip(allowed) {
        if !allowed {
            assert_eq!(before, after);
        }
    }
    if case.changed_rows == 0 {
        assert_eq!(after, case.before);
    }
}

#[test]
fn qualify_indexed_mixed_originals_and_literal_images_before_normal_writer() {
    let mut originals = 0;
    let mut expectations = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            shape(&case, recipe, &case.before, false);
            qualify(&case, &case.before, &case.rows);
            originals += 1;
            if let Some(after) = &case.expected {
                complement(&case, after);
                shape(&case, recipe, after, true);
                qualify(&case, after, case.final_rows.as_ref().unwrap());
                expectations += 1;
            } else {
                assert_eq!(recipe, Recipe::CounterUniqueConflict);
                assert!(case.unique);
                assert_eq!(
                    (case.updates[0].expected, case.updates[0].replacement),
                    (1000, 999)
                );
                assert!(case.final_rows.is_none());
            }
        }
    }
    assert_eq!((originals, expectations), (14, 12));
}

#[path = "normal_before_change_indexed_mixed_sources/export.rs"]
mod export;

#[path = "normal_before_change_indexed_mixed_sources/anchors.rs"]
mod anchors;
