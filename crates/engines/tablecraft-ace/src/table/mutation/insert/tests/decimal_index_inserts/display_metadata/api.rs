use super::*;
use crate::{EditError, RowInsert, WriteBlocker};

#[test]
fn numeric_display_places_preserve_exact_decimal_coefficient_and_declared_scale() {
    for code in [5, 6] {
        for places in [0, 15, 255] {
            let case = source(code, places);
            let rows = additions();
            let expected = model::expected(&case, &rows);
            let requests = rows
                .iter()
                .map(|row| RowInsert::new(row.values(case.settings)))
                .collect::<Vec<_>>();
            let candidate = original::capture(&case.before)
                .preview_ace16_insert_rows("Items", &requests, &mut original::budget(), &|| false)
                .unwrap();
            assert_eq!(candidate.candidate_bytes(), expected);

            let wrong_scale = [RowInsert::new(vec![
                Value::Int32(4),
                Value::Decimal(Decimal::new(-12345, 2).unwrap()),
            ])];
            assert_eq!(
                original::capture(&case.before)
                    .preview_ace16_insert_rows(
                        "Items",
                        &wrong_scale,
                        &mut original::budget(),
                        &|| false
                    )
                    .err(),
                Some(EditError::Unsupported(WriteBlocker::Column))
            );
        }
    }
}
