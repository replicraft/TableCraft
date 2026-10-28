//! SOURCE zero pairs and fresh map rows are independent of staged identities.
use super::*;
use crate::table::mutation::{
    append::State,
    insert::long_values::DeclarationBinding,
    insert::tests::long_value_inserts::{Family, initialization::model as initialization},
};

fn prepared(code: u8) -> (initialization::Case, NativeImage, Staged, Vec<RowInsert>) {
    let case = initialization::case(code, Family::Ole, Value::Binary(vec![0x92; 65]));
    assert_eq!(&case.before[10 * PAGE + 500..10 * PAGE + 508], &[0; 8]);
    checked(&case.before, 3);
    checked(&case.expected, 4);
    for (bytes, expected) in [
        (&case.before, initialization::rows(Family::Ole, None)),
        (
            &case.expected,
            initialization::rows(Family::Ole, Some(&case.value)),
        ),
    ] {
        let snapshot = crate::read_table(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(snapshot.rows(), expected);
    }
    initialization::checked_maps(&case.expected);
    let requests = vec![RowInsert::new(vec![
        Value::Int32(10),
        Value::Int32(100),
        case.value.clone(),
    ])];
    let (image, stage) = staged(&case.before, &requests);
    let after = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
    assert_eq!(after, case.expected);
    assert_eq!(stage.long_values.qualified_zero_columns, [2]);
    assert_eq!(stage.long_values.initializations.len(), 1);
    assert_eq!(stage.long_values.initializations[0].number, 18);
    assert_eq!(
        prove(
            &image,
            "Items",
            &requests,
            &stage,
            &after,
            &mut budget(),
            &|| false
        ),
        Ok(()),
    );
    (case, image, stage, requests)
}

#[test]
fn declaration_before_after_span_and_column_are_source_bound() {
    for code in [5, 6] {
        let (case, image, mut stage, requests) = prepared(code);
        let actual = &stage.long_values.initializations[0];
        let DeclarationBinding::ZeroPair {
            offset,
            before,
            after,
        } = &actual.binding
        else {
            panic!("literal source contains an explicit zero pair");
        };
        let original = (actual.number, actual.column, *offset, *before, *after);
        for mutation in 0..5 {
            let actual = &mut stage.long_values.initializations[0];
            let DeclarationBinding::ZeroPair {
                offset,
                before,
                after,
            } = &mut actual.binding
            else {
                unreachable!();
            };
            match mutation {
                0 => before[0] = 1,
                1 => after[0] ^= 1,
                2 => *offset += 1,
                3 => actual.column = 1,
                4 => actual.number = 19,
                _ => unreachable!(),
            }
            assert_eq!(
                prove(
                    &image,
                    "Items",
                    &requests,
                    &stage,
                    &case.expected,
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation),
                "code {code}, declaration mutation {mutation}",
            );
            let actual = &mut stage.long_values.initializations[0];
            actual.number = original.0;
            actual.column = original.1;
            actual.binding = DeclarationBinding::ZeroPair {
                offset: original.2,
                before: original.3,
                after: original.4,
            };
        }
    }
}

#[test]
fn whole_container_membership_header_and_unused_bytes_are_certified() {
    for code in [5, 6] {
        for offset in [4, 12, 3940, 4027 + 5 + 18 / 8, 3958 + 5 + 18 / 8] {
            let (case, image, mut stage, requests) = prepared(code);
            let actual = &mut stage.long_values.initializations[0];
            actual.bytes[offset] ^= 1;
            let changed = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
            assert_ne!(changed, case.expected);
            assert_eq!(
                prove(
                    &image,
                    "Items",
                    &requests,
                    &stage,
                    &changed,
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation),
                "code {code}, container offset {offset}",
            );
        }
    }
}

#[test]
fn map_container_cannot_be_substituted_with_ordinary_data_reservation() {
    for code in [5, 6] {
        let (case, image, mut stage, requests) = prepared(code);
        let body = &stage.long_values.pages[0];
        assert_eq!(body.number, 19);
        let mut replacement = State::new(&image, stage.data.append.owner);
        replacement
            .reserve_data(18, &mut budget(), &|| false)
            .unwrap();
        replacement
            .reserve_long_value(
                19,
                body.column,
                body.owned,
                body.candidate,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        replacement.map_edits = std::mem::take(&mut stage.data.append.map_edits);
        assert_eq!(replacement.length, stage.data.append.length);
        assert_eq!(
            replacement.reservations_len(),
            stage.data.append.reservations_len()
        );
        stage.data.append = replacement;
        assert_eq!(
            prove(
                &image,
                "Items",
                &requests,
                &stage,
                &case.expected,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation),
        );
    }
}

#[test]
fn zero_qualification_and_initialization_are_separate_from_inline_noop() {
    for code in [5, 6] {
        let (case, image, mut stage, _) = prepared(code);
        let initializer = stage.long_values.initializations.remove(0);
        let requests = [];
        let (_, mut inline) = staged(&case.before, &requests);
        let unchanged = compose::compose(&image, &inline, &mut budget(), &|| false).unwrap();
        assert_eq!(unchanged, case.before);
        assert_eq!(inline.long_values.qualified_zero_columns, [2]);
        assert!(inline.long_values.initializations.is_empty());
        assert_eq!(
            prove(
                &image,
                "Items",
                &requests,
                &inline,
                &unchanged,
                &mut budget(),
                &|| false
            ),
            Ok(()),
        );
        inline.long_values.qualified_zero_columns.clear();
        assert_eq!(
            prove(
                &image,
                "Items",
                &requests,
                &inline,
                &unchanged,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation),
        );
        inline.long_values.qualified_zero_columns.push(2);
        inline.long_values.initializations.push(initializer);
        assert_eq!(
            prove(
                &image,
                "Items",
                &requests,
                &inline,
                &unchanged,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation),
        );
    }
}
