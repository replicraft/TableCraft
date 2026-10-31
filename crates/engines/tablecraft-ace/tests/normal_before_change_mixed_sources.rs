//! Ordinary all-scalar mixed SOURCE gate; no new normal write service required.
#![cfg(feature = "journal")]
use std::io::Cursor;
use tablecraft_ace::{
    BeforeEventGuardError, BeforeEventIntents, BeforeGuardUnsupported, EditError, IndexCheckStatus,
    MacroProgramProfile, NativeImage, PayloadPresence, RuleCheckStatus, TableExtraCoverage,
    WriteBlocker, check_table, check_table_constraints, read_table, read_table_index,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::{
    canonical_fixed_creation as fixed,
    canonical_mixed_before_change::{self as model, Case, Recipe},
    canonical_variable_creation as variable,
};

const SCOPE: CatalogScope = CatalogScope::new(100);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn records(page: &[u8]) -> Vec<&[u8]> {
    let count = usize::from(u16::from_le_bytes(page[12..14].try_into().unwrap()));
    (0..count)
        .map(|slot| {
            let at = 14 + 2 * slot;
            let start = usize::from(u16::from_le_bytes(page[at..at + 2].try_into().unwrap()));
            let end = if slot == 0 {
                4096
            } else {
                usize::from(u16::from_le_bytes(page[at - 2..at].try_into().unwrap()))
            };
            assert_eq!(start & 0xe000, 0);
            &page[start..end]
        })
        .collect()
}

fn original_layout(case: &Case, recipe: Recipe) {
    if matches!(recipe, Recipe::FixedSequential | Recipe::FixedRestore) {
        let seed = fixed::source(fixed::Case::SignedZero);
        assert_eq!(
            &case.before[7 * 4096..9 * 4096],
            &seed.bytes[7 * 4096..9 * 4096]
        );
        assert_eq!(case.rows, seed.rows);
        return;
    }
    let seed = variable::source(if recipe == Recipe::CounterNullEmpty {
        variable::Case::NullAndEmpty
    } else {
        variable::Case::Unicode
    });
    let old = records(&seed.bytes[8 * 4096..9 * 4096]);
    let new = records(&case.before[8 * 4096..9 * 4096]);
    assert_eq!(old.len(), new.len());
    for (ordinal, (old, new)) in old.iter().zip(new).enumerate() {
        assert_eq!(&new[..2], &6_u16.to_le_bytes());
        assert_eq!(&new[2..6], &old[2..6]);
        assert_eq!(&new[6..10], &((ordinal as i32 + 1) * 1000).to_le_bytes());
        assert_eq!(new.len(), old.len() + 4);
        let old_footer = old.len() - 11;
        let new_footer = new.len() - 11;
        assert_eq!(&new[10..new_footer], &old[6..old_footer]);
        for word in 0..4 {
            let old_value = u16::from_le_bytes(
                old[old_footer + 2 * word..old_footer + 2 * word + 2]
                    .try_into()
                    .unwrap(),
            );
            assert_eq!(
                &new[new_footer + 2 * word..new_footer + 2 * word + 2],
                &(old_value + 4).to_le_bytes()
            );
        }
        assert_eq!(&new[new.len() - 3..new.len() - 1], &3_u16.to_le_bytes());
        assert_eq!(new[new.len() - 1], old[old.len() - 1] | 0x20);
    }
    let td = &case.before[7 * 4096..8 * 4096];
    assert_eq!(
        &td[20..24],
        &(if case.counter {
            case.rows.len() as u32
        } else {
            0
        })
        .to_le_bytes()
    );
    assert_eq!(&td[41..47], &[6, 0, 3, 0, 6, 0]);
    let start = if case.primary { 75 } else { 63 };
    for column in 0..5 {
        let source = &seed.bytes[7 * 4096 + 63 + 25 * column..7 * 4096 + 63 + 25 * (column + 1)];
        let actual = &td[start + 25 * column..start + 25 * (column + 1)];
        for byte in 0..25 {
            let expected = if column == 0 && byte == 15 && case.counter {
                7
            } else {
                source[byte]
            };
            assert_eq!(actual[byte], expected);
        }
    }
    let score = &td[start + 125..start + 150];
    assert_eq!(score[0], 4);
    assert_eq!(&score[5..11], &[5, 0, 3, 0, 5, 0]);
    assert_eq!(&score[15..17], &[3, 0]);
    assert_eq!(&score[21..25], &[4, 0, 4, 0]);
    let maps = records(&case.before[3 * 4096..4 * 4096]);
    let old_maps = records(&seed.bytes[3 * 4096..4 * 4096]);
    assert_eq!(maps.len(), if case.primary { 17 } else { 16 });
    assert_eq!(&maps[..16], &old_maps[..]);
    if case.primary {
        assert_eq!(maps[16], &[0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0]);
    }
    assert_eq!(&case.before[4096..3 * 4096], &seed.bytes[4096..3 * 4096]);
    assert_eq!(
        &case.before[5 * 4096..7 * 4096],
        &seed.bytes[5 * 4096..7 * 4096]
    );
}

fn qualify(case: &Case, bytes: &[u8], rows: &[Vec<Value>]) {
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap();
    let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), rows.len());
    assert_eq!(checked.indexes().len(), usize::from(case.primary));
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    let read = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(read.rows(), rows);
    let target = &checked.schema().columns()[case.target];
    assert_eq!(
        (
            target.native_type(),
            target.flags(),
            target.extended_flags(),
            target.storage_length()
        ),
        (4, 3, 0, 4)
    );
    assert!(target.fixed());
    if case.counter {
        let id = &checked.schema().columns()[0];
        assert_eq!(
            (
                id.name(),
                id.native_type(),
                id.flags(),
                id.extended_flags(),
                id.storage_length()
            ),
            ("Id", 4, 7, 0, 4)
        );
    }
    if case.primary {
        let index = checked.indexes()[0].index();
        assert_eq!(
            (
                index.name(),
                index.data_number(),
                index.root_page(),
                index.usage_map()
            ),
            ("PrimaryId", 0, 9, (3 << 8) | 16)
        );
        assert_eq!(index.columns().len(), 1);
        assert_eq!(index.columns()[0].number(), 0);
        assert!(index.constraints().unwrap().primary_key());
        let indexed = read_table_index(
            &mut image.reader(),
            SCOPE,
            "Items",
            "PrimaryId",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(indexed.rows(), rows);
    }
    let constraints =
        check_table_constraints(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(constraints.rows(), rows.len());
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
    let group = &coverage.table_extra().groups()[0];
    assert_eq!(group.kind(), 3);
    let properties = group.properties().unwrap();
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
    // This public complete-row logical API deliberately remains all-updatable
    // Int32. Its refusal is recorded, never bypassed or called normal admission.
    let error = image
        .prepare_native_before_change_fields(
            SCOPE.object_id(7),
            BeforeEventIntents::FixedUpdate {
                column: case.column,
                updates: &[],
            },
            &mut budget(),
            &|| false,
        )
        .err()
        .expect("old complete-row logical profile stays strict");
    if case.counter {
        assert!(matches!(
            error,
            BeforeEventGuardError::Intent(EditError::Unsupported(WriteBlocker::Column))
        ));
    } else {
        assert!(matches!(
            error,
            BeforeEventGuardError::Unsupported(BeforeGuardUnsupported::SourceProfile)
        ));
    }
    assert_eq!(image.as_bytes(), bytes);
}

#[test]
fn qualify_mixed_scalar_originals_and_literal_expectations_before_projected_normal_writer() {
    let mut originals = 0;
    let mut expectations = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            original_layout(&case, recipe);
            qualify(&case, &case.before, &case.rows);
            originals += 1;
            if let Some(after) = &case.expected {
                qualify(&case, after, case.final_rows.as_ref().unwrap());
                assert_eq!(&after[..8 * 4096], &case.before[..8 * 4096]);
                if case.primary {
                    assert_eq!(&after[9 * 4096..], &case.before[9 * 4096..]);
                }
                expectations += 1;
            } else {
                assert!(case.refusal.is_some());
            }
        }
    }
    assert_eq!((originals, expectations), (24, 16));
}

#[path = "normal_before_change_mixed_sources/export.rs"]
mod export;

#[path = "normal_before_change_mixed_sources/anchors.rs"]
mod anchors;
