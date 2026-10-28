//! Container order follows physical variable ordinals, independently of requests.
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
fn every_container_precedes_bodies_even_when_first_request_uses_the_later_ordinal() {
    for code in [5, 6] {
        let mut before = composition::model::source_and_expected(code, false).before;
        assert_eq!(&before[10 * PAGE + 533..10 * PAGE + 535], &[3, 0]);
        assert_eq!(&before[10 * PAGE + 543..10 * PAGE + 545], &[2, 0]);
        before[10 * PAGE + 535..10 * PAGE + 543].fill(0);
        before[10 * PAGE + 545..10 * PAGE + 553].fill(0);
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
        let profile = Profile::bind(checked.schema(), &mut original::budget(), &|| false).unwrap();
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
        assert_eq!(plan.qualified_zero_columns, [3, 2]);
        let containers = plan
            .initializations
            .iter()
            .map(|entry| {
                let DeclarationBinding::ZeroPair { offset, .. } = &entry.binding else {
                    panic!("the source has explicit zero pairs");
                };
                (entry.number, entry.column, *offset)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            containers,
            [(18, 3, 10 * PAGE + 535), (19, 2, 10 * PAGE + 545)]
        );
        let bodies = plan
            .pages
            .iter()
            .map(|page| (page.number, page.column))
            .collect::<Vec<_>>();
        assert_eq!(bodies, [(20, 2), (21, 3)]);
        assert!(plan.metadata.is_empty());
        assert_eq!(append.length, 22 * PAGE);
        assert_eq!(append.reservations_len(), 4);
        assert_eq!(image.as_bytes(), before);
    }
}
