//! Read-only qualification of a separate present-Id/untouched-NULL companion.
#![cfg(feature = "journal")]

use std::io::Cursor;
use tablecraft_ace::{
    IndexCheckStatus, MacroProgramProfile, NativeImage, PayloadPresence, RuleCheckStatus,
    TableExtraCoverage, check_table, check_table_constraints, read_table, read_table_index,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::{
    canonical_nullable_index_before_change as original,
    canonical_nullable_index_id_before_change as model,
};

const SCOPE: CatalogScope = CatalogScope::new(110);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn complement(case: &model::Case) {
    let page = 8 * 4096;
    let selected = page
        + usize::from(u16::from_le_bytes(
            case.before[page + 14..page + 16].try_into().unwrap(),
        ));
    let null = page
        + usize::from(u16::from_le_bytes(
            case.before[page + 18..page + 20].try_into().unwrap(),
        ));
    let null_end = page
        + usize::from(u16::from_le_bytes(
            case.before[page + 16..page + 18].try_into().unwrap(),
        ));
    assert_eq!(case.expected.len(), case.before.len());
    let mut allowed = vec![false; case.before.len()];
    allowed[selected + 2..selected + 10].fill(true);
    allowed[9 * 4096 + 480..9 * 4096 + 503].fill(true);
    for ((before, after), allowed) in case.before.iter().zip(&case.expected).zip(allowed) {
        if !allowed {
            assert_eq!(before, after);
        }
    }
    assert_eq!(&case.expected[null..null_end], &case.before[null..null_end]);
    assert_eq!(&case.expected[null + 2..null + 6], &3_i32.to_le_bytes());
    assert_eq!(case.expected[null_end - 1] & 1, 0);
}

#[test]
fn qualify_present_id_and_macro_score_images_before_nullable_index_target_permission() {
    let mut views = 0;
    for header in [5, 6] {
        let case = model::source(header);
        assert_eq!(case.before, original::source(header).before);
        complement(&case);
        assert_eq!(
            (
                case.column,
                case.update.ordinal,
                case.update.expected,
                case.update.replacement
            ),
            ("Id", 0, 1, 7)
        );
        for (bytes, rows, after) in [
            (&case.before, &case.rows, false),
            (&case.expected, &case.final_rows, true),
        ] {
            assert_eq!(bytes.len(), 10 * 4096);
            let image =
                NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false)
                    .unwrap();
            let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
            assert_eq!(checked.rows(), 3);
            assert_eq!(checked.indexes().len(), 1);
            assert_eq!(checked.indexes()[0].status(), IndexCheckStatus::Checked);
            let id = &checked.schema().columns()[0];
            let score = &checked.schema().columns()[5];
            assert_eq!(
                (
                    id.native_type(),
                    id.flags(),
                    id.extended_flags(),
                    id.storage_length()
                ),
                (4, 3, 0, 4)
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
            let index = checked.indexes()[0].index();
            assert_eq!(
                (
                    index.name(),
                    index.data_number(),
                    index.root_page(),
                    index.usage_map(),
                    index.flags()
                ),
                ("PrimaryId", 0, 9, (3 << 8) | 16, 0x80)
            );
            assert!(!index.constraints().unwrap().primary_key());
            assert!(!index.constraints().unwrap().unique());
            assert!(!index.constraints().unwrap().required());
            assert!(!index.constraints().unwrap().ignore_nulls());
            assert_eq!(index.columns().len(), 1);
            assert_eq!(
                (index.columns()[0].number(), index.columns()[0].descending()),
                (0, false)
            );
            let table = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
            assert_eq!(table.rows(), rows);
            assert_eq!(table.rows()[2][0], Value::Null);
            assert_eq!(table.rows()[0][0], Value::Int32(if after { 7 } else { 1 }));
            assert_eq!(
                table.rows()[0][5],
                Value::Int32(if after { 2007 } else { 1000 })
            );
            let indexed = read_table_index(
                &mut image.reader(),
                SCOPE,
                "Items",
                "PrimaryId",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let order = if after { [2, 1, 0] } else { [2, 0, 1] };
            assert_eq!(indexed.rows(), &order.map(|ordinal| rows[ordinal].clone()));
            let constraints = check_table_constraints(
                &mut image.reader(),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
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
            let properties = coverage.table_extra().groups()[0].properties().unwrap();
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
            assert_eq!(
                &bytes[9 * 4096..9 * 4096 + 480],
                &case.before[9 * 4096..9 * 4096 + 480]
            );
            assert_eq!(&bytes[9 * 4096 + 2..9 * 4096 + 4], &3593_u16.to_le_bytes());
            assert_eq!(&bytes[9 * 4096 + 24..9 * 4096 + 26], &0_u16.to_le_bytes());
            let mut bitmap = [0; 453];
            for end in [5, 14, 23] {
                bitmap[end / 8] |= 1 << (end % 8);
            }
            assert_eq!(&bytes[9 * 4096 + 27..9 * 4096 + 480], &bitmap);
            assert_eq!(&bytes[9 * 4096 + 480..9 * 4096 + 485], &[0, 0, 0, 8, 2]);
            assert_eq!(&bytes[9 * 4096 + 503..], &case.before[9 * 4096 + 503..]);
            assert_eq!(image.as_bytes(), bytes);
            views += 1;
        }
    }
    assert_eq!(views, 4);
}

#[path = "normal_before_change_nullable_id_sources/export.rs"]
mod export;

#[path = "normal_before_change_nullable_id_sources/anchors.rs"]
mod anchors;
