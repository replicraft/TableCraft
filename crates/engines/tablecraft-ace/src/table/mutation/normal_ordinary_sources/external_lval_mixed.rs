//! Read-only source qualification of full mixed-retirement literal images.
use crate::{
    IndexCheckStatus, LongValueCheckStatus, NativeImage, check_table, check_table_constraints,
    check_table_long_values, inspect_table_index_counter, read_table, read_table_index,
};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_external_lval_mixed as fixtures;

const SCOPE: CatalogScope = CatalogScope::new(193);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires independently constructed mixed external source20 images"]
fn mixed_external_retirement_sources_have_exact_bytes_and_full_ordinary_source() {
    let cases = fixtures::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    });
    let mut contexts = 0;
    for case in cases {
        assert_eq!(
            case.after,
            fixtures::expected(&case.before, case.header, case.recipe)
        );
        let image =
            NativeImage::capture(&mut Cursor::new(&case.after), SCOPE, &mut budget(), &|| {
                false
            })
            .unwrap();
        let table = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
        assert_eq!(table.schema().definition_page(), case.definition as u32);
        assert_eq!(table.rows(), case.rows);
        assert_eq!(table.schema().columns().len(), 4);
        let columns = table.schema().columns();
        assert_eq!(
            (
                columns[2].native_type(),
                columns[2].flags(),
                columns[2].extended_flags()
            ),
            (11, 2, 0)
        );
        assert_eq!(
            (
                columns[3].native_type(),
                columns[3].flags(),
                columns[3].extended_flags()
            ),
            (12, 2, u8::from(case.profile.ends_with("-compressed")))
        );
        let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        })
        .unwrap();
        assert_eq!(checked.rows(), 6);
        assert_eq!(checked.indexes().len(), 4);
        assert!(
            checked
                .indexes()
                .iter()
                .all(|index| index.status() == IndexCheckStatus::Checked)
        );
        for (name, order, history) in [
            ("PrimaryID", [0, 1, 2, 3, 4, 5], 7),
            ("PrimaryAlias", [0, 1, 2, 3, 4, 5], 7),
            ("GroupOrder", [0, 4, 1, 2, 3, 5], 5),
            ("GroupAlias", [0, 4, 1, 2, 3, 5], 5),
        ] {
            let read = read_table_index(
                &mut image.reader(),
                SCOPE,
                "Items",
                name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let expected = order.map(|index| case.rows[index].clone());
            assert_eq!(read.rows(), expected);
            assert_eq!(
                inspect_table_index_counter(
                    &mut image.reader(),
                    SCOPE,
                    "Items",
                    name,
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .value(),
                history
            );
        }
        assert_eq!(
            check_table_constraints(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap()
            .rows(),
            6
        );
        let storage =
            check_table_long_values(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(storage.rows(), 6);
        assert_eq!(storage.columns().len(), 2);
        for (position, column) in storage.columns().iter().enumerate() {
            let retired = usize::from(match position {
                0 => matches!(
                    case.recipe,
                    "clear-blob3-grow-memo5" | "clear-blob3-grow-memo3"
                ),
                1 => matches!(
                    case.recipe,
                    "clear-memo3-reuse-blob3" | "clear-memo3-shrink-blob3"
                ),
                _ => unreachable!(),
            });
            assert_eq!(column.column_number(), position + 2);
            assert_eq!(column.status(), LongValueCheckStatus::Checked);
            assert_eq!(column.owned_pages(), 5 - retired);
            assert_eq!(column.candidate_pages(), 1);
            assert_eq!(
                column.owned_rows(),
                (if position == 0 { 6 } else { 5 }) - retired
            );
            assert_eq!(
                column.distinct_chunks(),
                (if position == 0 { 5 } else { 4 }) - retired
            );
            assert_eq!(column.orphan_rows(), 1);
            assert_eq!(column.shared_references(), 0);
            assert_eq!(column.shared_owned_pages(), 0);
        }
        // Private Source identity stays in the engine unit crate. This is a
        // read-only certificate check, never an edit planner or preview.
        super::super::normal_ordinary::capture(&image, "Items", &mut budget(), &|| false).unwrap();
        image
            .check_native_ordinary_fixed_table("Items", &mut budget(), &|| false)
            .unwrap();
        assert_eq!(image.as_bytes(), case.after);
        contexts += 1;
    }
    assert_eq!(contexts, 20);
}
