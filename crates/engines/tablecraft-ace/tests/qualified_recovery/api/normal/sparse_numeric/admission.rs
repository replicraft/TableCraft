//! Authored negative derivatives; these are not additional qualified source files.
use super::{budget, cases, image};
use tablecraft_ace::{EditError, InspectError, RecoverableEditError, RowInsert, WriteBlocker};

#[test]
#[ignore = "requires source-qualified sparse action originals"]
fn native_and_stored_identifier_corruptions_refuse_before_action_publication() {
    for case in cases()
        .into_iter()
        .filter(|case| case.recipe == "insert-zero")
    {
        let first = case.definition as usize * 4096 + 99;
        for (at, value) in [(first + 25 + 5, 0_u16), (first + 50 + 5, case.maximum())] {
            let mut negative = case.before.clone();
            negative[at..at + 2].copy_from_slice(&value.to_le_bytes());
            let error = image(&negative)
                .plan_recoverable_ordinary_row_inserts(
                    case.table,
                    vec![RowInsert::new(case.request())],
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap();
            assert!(matches!(
                error,
                RecoverableEditError::Edit(EditError::Inspect(InspectError::Malformed))
            ));
        }
        // Distinct native numbers remain in range but violate declared order.
        let mut negative = case.before.clone();
        for (position, native) in [(1, case.numbers()[2]), (2, case.numbers()[1])] {
            let at = first + position * 25 + 5;
            negative[at..at + 2].copy_from_slice(&(native as u16).to_le_bytes());
        }
        let error = image(&negative)
            .plan_recoverable_ordinary_row_inserts(
                case.table,
                vec![RowInsert::new(case.request())],
                &mut budget(),
                &|| false,
            )
            .err()
            .unwrap();
        assert!(matches!(
            error,
            RecoverableEditError::Edit(EditError::Unsupported(WriteBlocker::ActiveContent))
        ));
        for ids in [[0_u16, 0, 4], [0, 4, 3], [0, u16::MAX, 9]] {
            let mut negative = case.before.clone();
            for (position, id) in ids.into_iter().enumerate() {
                let at = first + position * 25 + 9;
                negative[at..at + 2].copy_from_slice(&id.to_le_bytes());
            }
            let error = image(&negative)
                .plan_recoverable_ordinary_row_inserts(
                    case.table,
                    vec![RowInsert::new(case.request())],
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap();
            assert!(matches!(
                error,
                RecoverableEditError::Edit(EditError::Unsupported(WriteBlocker::ActiveContent))
            ));
        }
    }
}
