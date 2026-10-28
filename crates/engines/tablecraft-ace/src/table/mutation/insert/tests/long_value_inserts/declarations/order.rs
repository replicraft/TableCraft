//! One TD extension covers both absent columns in actual variable order.
use super::super::{PAGE, composition, original};
use crate::{
    check_table,
    table::mutation::insert::{
        long_values::{self, DeclarationBinding},
        record::{Profile, input},
    },
};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

#[test]
fn missing_and_zero_pairs_share_tail_growth_before_request_ordered_bodies() {
    for code in [5, 6] {
        for mixed in [false, true] {
            let mut before = composition::model::source_and_expected(code, false).before;
            let definition = 10 * PAGE;
            let terminal = if mixed { 543 } else { 533 };
            let old_length = terminal + 2;
            let old_free = (4096 - old_length - 8) as u16;
            if mixed {
                before[definition + 535..definition + 543].fill(0);
            }
            before[definition + terminal..definition + old_length].copy_from_slice(&[255, 255]);
            before[definition + 8..definition + 12]
                .copy_from_slice(&(old_length as u32).to_le_bytes());
            before[definition + 2..definition + 4].copy_from_slice(&old_free.to_le_bytes());
            original::checked(&before, 1, 4);

            let image = original::capture(&before);
            let checked = check_table(
                &mut image.reader(),
                CatalogScope::new(1),
                "Items",
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
            let profile =
                Profile::bind(checked.schema(), &mut original::budget(), &|| false).unwrap();
            assert_eq!(profile.variables(), &[3, 2]);
            let values = [
                vec![
                    Value::Int32(10),
                    Value::Null,
                    Value::Binary(vec![0x92; 65]),
                    Value::Null,
                ],
                vec![
                    Value::Int32(11),
                    Value::Null,
                    Value::Null,
                    Value::Text("Ā".repeat(33)),
                ],
            ];
            let mut inputs = values
                .iter()
                .map(|row| {
                    input::prepare(
                        checked.schema(),
                        profile.variables(),
                        row,
                        &mut original::budget(),
                        &|| false,
                    )
                    .unwrap()
                })
                .collect::<Vec<_>>();

            let (plan, append) = long_values::prepare(
                &image,
                &checked,
                profile.variables(),
                &mut inputs,
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                plan.qualified_zero_columns,
                if mixed { vec![3] } else { vec![] }
            );
            let mut qualified_missing = plan.qualified_missing_columns.clone();
            qualified_missing.sort_unstable();
            assert_eq!(qualified_missing, if mixed { vec![2] } else { vec![2, 3] });
            let containers = plan
                .initializations
                .iter()
                .map(|entry| {
                    if mixed && entry.column == 3 {
                        let DeclarationBinding::ZeroPair {
                            offset,
                            before,
                            after,
                        } = &entry.binding
                        else {
                            panic!("Memo has the original zero pair");
                        };
                        assert_eq!(*offset, definition + 535);
                        assert_eq!(*before, [0; 8]);
                        let expected = [
                            (18_u32 << 8).to_le_bytes(),
                            ((18_u32 << 8) | 1).to_le_bytes(),
                        ]
                        .concat();
                        assert_eq!(after.as_slice(), expected);
                    } else {
                        assert!(matches!(
                            entry.binding,
                            DeclarationBinding::MissingDeclaration
                        ));
                    }
                    (entry.number, entry.column)
                })
                .collect::<Vec<_>>();
            assert_eq!(containers, [(18, 3), (19, 2)]);
            let bodies = plan
                .pages
                .iter()
                .map(|page| (page.number, page.column))
                .collect::<Vec<_>>();
            assert_eq!(bodies, [(20, 2), (21, 3)]);

            let growth = plan.definition_growth.as_ref().unwrap();
            assert_eq!(growth.definition, 10);
            assert_eq!(growth.offset, definition + terminal);
            assert_eq!((growth.old_length, growth.new_length), (old_length, 555));
            assert_eq!((growth.old_free, growth.new_free), (old_free, 3533));
            assert_eq!(
                growth.before,
                before[definition + terminal..definition + 555]
            );
            let mut expected = Vec::new();
            if !mixed {
                expected.extend([3, 0]);
                expected.extend((18_u32 << 8).to_le_bytes());
                expected.extend(((18_u32 << 8) | 1).to_le_bytes());
            }
            expected.extend([2, 0]);
            expected.extend((19_u32 << 8).to_le_bytes());
            expected.extend(((19_u32 << 8) | 1).to_le_bytes());
            expected.extend([255, 255]);
            assert_eq!(growth.after, expected);
            assert_eq!(append.length, 22 * PAGE);
            assert_eq!(append.reservations_len(), 4);
            assert_eq!(image.as_bytes(), before);
        }
    }
}
