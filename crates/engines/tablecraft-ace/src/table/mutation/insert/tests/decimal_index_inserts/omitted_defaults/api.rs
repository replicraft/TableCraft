use super::*;
use crate::{EditError, InsertFieldValue, OmittedRowInsert, WriteBlocker};
use tablecraft_core::values::Value;

#[test]
fn omitted_decimal_literals_preserve_exact_domain_without_float_or_rounding() {
    for code in [5, 6] {
        for (scale, text, coefficient) in [
            (4, "12.3400", 123400_i128),
            (28, "0.125", 1_250_000_000_000_000_000_000_000_000_i128),
        ] {
            let case = source(code, 28, scale, text);
            let row = Row {
                id: 4,
                amount: Some(coefficient),
                guid: None,
            };
            let expected = model::expected(&case, std::slice::from_ref(&row));
            let candidate = original::capture(&case.before)
                .preview_ace16_insert_rows_with_omissions(
                    "Items",
                    &[OmittedRowInsert::new(vec![
                        InsertFieldValue::Supplied(Value::Int32(4)),
                        InsertFieldValue::Omitted,
                    ])],
                    &mut original::budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(candidate.candidate_bytes().len(), expected.len());
            assert_eq!(
                candidate
                    .candidate_bytes()
                    .iter()
                    .zip(&expected)
                    .position(|(left, right)| left != right),
                None
            );
        }
        for (precision, scale, text) in [(3, 2, "10.00"), (28, 4, "1.23456")] {
            let case = source(code, precision, scale, text);
            assert_eq!(
                original::capture(&case.before)
                    .preview_ace16_insert_rows_with_omissions(
                        "Items",
                        &[OmittedRowInsert::new(vec![
                            InsertFieldValue::Supplied(Value::Int32(4)),
                            InsertFieldValue::Omitted
                        ])],
                        &mut original::budget(),
                        &|| false,
                    )
                    .err(),
                Some(EditError::Unsupported(WriteBlocker::Column))
            );
        }
    }
}
