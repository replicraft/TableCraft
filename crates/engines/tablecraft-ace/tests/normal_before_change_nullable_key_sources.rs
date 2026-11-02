//! Existing readers qualify authored nullable key grammar before new writers.
#![cfg(feature = "journal")]

use std::{cmp::Ordering, io::Cursor};
use tablecraft_ace::{
    IndexCheckStatus, MacroProgramProfile, NativeImage, PayloadPresence, RuleCheckStatus,
    TableExtraCoverage, check_table, check_table_constraints, read_table, read_table_index,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_indexed_mixed_before_change as original,
    canonical_nullable_key_before_change::{self as model, Case, Recipe},
};

const SCOPE: CatalogScope = CatalogScope::new(109);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn location(bytes: &[u8], ordinal: usize) -> (usize, usize) {
    let word = 8 * 4096 + 14 + 2 * ordinal;
    let start = u16::from_le_bytes(bytes[word..word + 2].try_into().unwrap());
    let end = if ordinal == 0 {
        9 * 4096
    } else {
        8 * 4096
            + usize::from(u16::from_le_bytes(
                bytes[word - 2..word].try_into().unwrap(),
            ))
    };
    assert_eq!(start & 0xe000, 0);
    (8 * 4096 + usize::from(start), end)
}

fn original_complement(case: &Case, recipe: Recipe, header: u8) {
    let old = original::source(header, recipe.seed()).before;
    assert_eq!(old.len(), case.before.len());
    let mut allowed = vec![false; old.len()];
    for &ordinal in &case.null_rows {
        let (start, end) = location(&case.before, ordinal);
        allowed[end - 1] = true;
        assert_eq!(old[end - 1] ^ case.before[end - 1], 0x20);
        assert_eq!(
            &case.before[start + 6..start + 10],
            &((ordinal as i32 + 1) * 1000).to_le_bytes()
        );
        assert_eq!(case.before[end - 1] & 0x20, 0);
    }
    for &number in &case.owned {
        allowed[number as usize * 4096..(number as usize + 1) * 4096].fill(true);
    }
    if recipe == Recipe::ScalarDescending {
        let td = &old[7 * 4096..8 * 4096];
        let mut names_end = 63 + 2 * 12 + 6 * 25;
        for _ in 0..6 {
            names_end += 2 + usize::from(u16::from_le_bytes(
                td[names_end..names_end + 2].try_into().unwrap(),
            ));
        }
        let direction = 7 * 4096 + names_end + 52 + 6;
        assert_eq!(old[direction], 1);
        assert_eq!(case.before[direction], 0);
        allowed[direction] = true;
    }
    for ((old, new), allowed) in old.iter().zip(&case.before).zip(allowed) {
        if !allowed {
            assert_eq!(old, new);
        }
    }
}

fn shape(case: &Case, recipe: Recipe, bytes: &[u8], after: bool) {
    assert_eq!(bytes.len(), if case.node { 13 * 4096 } else { 11 * 4096 });
    assert_eq!(&bytes[3 * 4096..4 * 4096], &case.before[3 * 4096..4 * 4096]);
    assert_eq!(
        &bytes[9 * 4096..10 * 4096],
        &case.before[9 * 4096..10 * 4096]
    );
    for &ordinal in &case.null_rows {
        let (start, end) = location(bytes, ordinal);
        assert_eq!(bytes[end - 1] & 0x20, 0);
        assert_eq!(
            &bytes[start + 6..start + 10],
            &((ordinal as i32 + 1) * 1000).to_le_bytes()
        );
        if after {
            assert_eq!(&bytes[start..end], &case.before[start..end]);
        }
    }
    for &number in &case.owned {
        let number = number as usize;
        let node = case.node && number == 10;
        let lengths = match (recipe, number) {
            (Recipe::ScalarDescending, _) => vec![9, 9, 5],
            (Recipe::UniqueNullPair, _) => vec![5, 5, 9],
            (Recipe::CompositePartialNull, _) => vec![10, 14, 14],
            (Recipe::TreeNullFirst, 10) => vec![13],
            (Recipe::TreeNullFirst, 11) => vec![5, 9],
            (Recipe::TreeNullFirst, 12) => vec![9],
            (Recipe::TreeNullBound, 10) => vec![9],
            (Recipe::TreeNullBound, 11) => vec![5],
            (Recipe::TreeNullBound, 12) => vec![9, 9],
            _ => vec![5, 9, 9],
        };
        let active: usize = lengths.iter().sum();
        let page = &bytes[number * 4096..(number + 1) * 4096];
        assert_eq!(&page[..2], &[if node { 3 } else { 4 }, 1]);
        assert_eq!(&page[2..4], &((3616 - active) as u16).to_le_bytes());
        assert_eq!(&page[4..8], &7_u32.to_le_bytes());
        assert_eq!(&page[8..12], &[0; 4]);
        assert_eq!(&page[24..26], &0_u16.to_le_bytes());
        assert_eq!(page[26], u8::from(node));
        let links: [u32; 3] = match (case.node, number) {
            (true, 10) => [0, 0, 12],
            (true, 11) => [0, 12, 0],
            (true, 12) => [11, 0, 0],
            _ => [0, 0, 0],
        };
        for (position, value) in links.iter().enumerate() {
            assert_eq!(
                &page[12 + 4 * position..16 + 4 * position],
                &value.to_le_bytes()
            );
        }
        let mut bitmap = [0; 453];
        let mut end = 0;
        for length in lengths {
            end += length;
            bitmap[end / 8] |= 1 << (end % 8);
        }
        assert_eq!(&page[27..480], &bitmap);
        if after {
            assert_eq!(
                &page[480 + active..],
                &case.before[number * 4096 + 480 + active..(number + 1) * 4096]
            );
        } else {
            assert!(page[480 + active..].iter().all(|byte| *byte == 0xa7));
        }
    }
    let root = &bytes[10 * 4096..11 * 4096];
    match recipe {
        Recipe::ScalarAscending => assert_eq!(&root[480..485], &[0, 0, 0, 8, 2]),
        Recipe::ScalarDescending => assert_eq!(&root[498..503], &[0xff, 0, 0, 8, 2]),
        Recipe::UniqueNullPair => assert_eq!(&root[480..490], &[0, 0, 0, 8, 1, 0, 0, 0, 8, 2]),
        Recipe::CompositePartialNull => assert_eq!(
            &root[480..490],
            &[0, 0x80, 0x7f, 0xff, 0xff, 0xfc, 0, 0, 8, 2]
        ),
        Recipe::TreeNullBound => assert_eq!(&root[480..489], &[0, 0, 0, 8, 2, 0, 0, 0, 11]),
        Recipe::TreeNullFirst => {
            let bound = if after {
                [0x7f, 0x80, 0, 7, 0xd0, 0, 0, 8, 1, 0, 0, 0, 11]
            } else {
                [0x7f, 0x80, 0, 3, 0xe8, 0, 0, 8, 0, 0, 0, 0, 11]
            };
            assert_eq!(&root[480..493], &bound);
        }
    }
}

fn ordered(rows: &[Vec<Value>], components: &[(usize, bool)]) -> Vec<Vec<Value>> {
    fn value(cell: &Value) -> Option<i32> {
        match cell {
            Value::Null => None,
            Value::Int32(value) => Some(*value),
            _ => panic!("literal nullable I32"),
        }
    }
    let mut result = rows.iter().enumerate().collect::<Vec<_>>();
    result.sort_by(|(a_slot, a), (b_slot, b)| {
        for &(number, descending) in components {
            let compare = if descending {
                value(&b[number]).cmp(&value(&a[number]))
            } else {
                value(&a[number]).cmp(&value(&b[number]))
            };
            if compare != Ordering::Equal {
                return compare;
            }
        }
        a_slot.cmp(b_slot)
    });
    result.into_iter().map(|(_, row)| row.clone()).collect()
}

fn qualify(case: &Case, bytes: &[u8], rows: &[Vec<Value>]) {
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap();
    let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), 3);
    assert_eq!(checked.indexes().len(), 2);
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let sequential = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(sequential.rows(), rows);
    let id = &checked.schema().columns()[0];
    let score = &checked.schema().columns()[5];
    assert_eq!(
        (id.native_type(), id.flags(), id.storage_length()),
        (4, 7, 4)
    );
    assert_eq!(
        (
            score.native_type(),
            score.flags(),
            score.extended_flags(),
            score.storage_length()
        ),
        (4, 3, 0, 4)
    );
    let primary = checked.indexes()[0].index();
    assert_eq!(
        (
            primary.name(),
            primary.root_page(),
            primary.usage_map(),
            primary.flags()
        ),
        ("PrimaryId", 9, (3 << 8) | 16, 0x89)
    );
    assert!(primary.constraints().unwrap().primary_key());
    let selected = checked.indexes()[1].index();
    assert_eq!(
        (
            selected.name(),
            selected.data_number(),
            selected.root_page(),
            selected.usage_map()
        ),
        (case.index_name, 1, 10, (3 << 8) | 17)
    );
    assert_eq!(selected.flags(), if case.unique { 0x81 } else { 0x80 });
    assert_eq!(selected.constraints().unwrap().unique(), case.unique);
    assert!(!selected.constraints().unwrap().primary_key());
    assert!(!selected.constraints().unwrap().required());
    assert!(!selected.constraints().unwrap().ignore_nulls());
    assert_eq!(
        selected
            .columns()
            .iter()
            .map(|key| (key.number(), key.descending()))
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
    assert_eq!(indexed.rows().len(), 3);
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

fn expected_complement(case: &Case) {
    let (start, _) = location(&case.before, 0);
    let mut allowed = vec![false; case.before.len()];
    allowed[start + 6..start + 10].fill(true);
    for &number in &case.owned {
        allowed[number as usize * 4096..(number as usize + 1) * 4096].fill(true);
    }
    for ((old, new), allowed) in case.before.iter().zip(&case.expected).zip(allowed) {
        if !allowed {
            assert_eq!(old, new);
        }
    }
}

#[test]
fn qualify_untouched_nullable_key_originals_and_literal_images_before_new_writer() {
    let mut views = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            original_complement(&case, recipe, header);
            expected_complement(&case);
            for (bytes, rows, after) in [
                (&case.before, &case.rows, false),
                (&case.expected, &case.final_rows, true),
            ] {
                shape(&case, recipe, bytes, after);
                qualify(&case, bytes, rows);
                views += 1;
            }
        }
    }
    assert_eq!(views, 24);
}

#[path = "normal_before_change_nullable_key_sources/export.rs"]
mod export;

#[path = "normal_before_change_nullable_key_sources/anchors.rs"]
mod anchors;
