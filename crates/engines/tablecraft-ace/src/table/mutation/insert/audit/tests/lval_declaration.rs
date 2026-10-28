//! Exact source TD spare and typed bindings cannot be supplied by a staged plan.
use super::*;
use crate::table::mutation::insert::{
    long_values::DeclarationBinding,
    tests::long_value_inserts::{
        Family, declarations::model as declaration, initialization::model as zero,
    },
};

fn prepared(code: u8) -> (declaration::Case, NativeImage, Staged, Vec<RowInsert>) {
    let case = declaration::nonzero_spare(code, Family::Ole);
    for (bytes, expected) in [
        (&case.before, zero::rows(Family::Ole, None)),
        (&case.expected, zero::rows(Family::Ole, Some(&case.value))),
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
    zero::checked_maps(&case.expected);
    let requests = vec![RowInsert::new(vec![
        Value::Int32(10),
        Value::Int32(100),
        case.value.clone(),
    ])];
    let (image, stage) = staged(&case.before, &requests);
    let actual = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
    assert_eq!(actual, case.expected);
    assert_eq!(stage.long_values.qualified_missing_columns, [2]);
    assert!(stage.long_values.qualified_zero_columns.is_empty());
    assert!(matches!(
        stage.long_values.initializations[0].binding,
        DeclarationBinding::MissingDeclaration
    ));
    assert_eq!(
        prove(
            &image,
            "Items",
            &requests,
            &stage,
            &actual,
            &mut budget(),
            &|| false
        ),
        Ok(()),
    );
    (case, image, stage, requests)
}

#[test]
fn consumed_nonzero_spare_and_both_td_headers_have_exact_source_identity() {
    for code in [5, 6] {
        for mutation in 0..8 {
            let (case, image, mut stage, requests) = prepared(code);
            let growth = stage.long_values.definition_growth.as_mut().unwrap();
            assert_eq!(growth.before[2], 0x60);
            match mutation {
                0 => growth.before[2] ^= 1,
                1 => growth.offset += 1,
                2 => growth.old_length += 1,
                3 => growth.new_length += 1,
                4 => growth.old_free ^= 1,
                5 => growth.new_free ^= 1,
                6 => growth.definition = 11,
                7 => growth.before[0] = 0,
                _ => unreachable!(),
            }
            // The native-readable candidate remains healthy. These changes
            // concern source spans and role identity, not candidate corruption.
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
                "code {code}, source/header mutation {mutation}",
            );
        }
    }
}

#[test]
fn correlated_pair_reference_swap_cannot_rebind_equal_map_membership() {
    for code in [5, 6] {
        let (case, image, mut stage, requests) = prepared(code);
        let growth = stage.long_values.definition_growth.as_mut().unwrap();
        let owned: [u8; 4] = growth.after[2..6].try_into().unwrap();
        let candidate: [u8; 4] = growth.after[6..10].try_into().unwrap();
        growth.after[2..6].copy_from_slice(&candidate);
        growth.after[6..10].copy_from_slice(&owned);
        let changed = compose::compose(&image, &stage, &mut budget(), &|| false).unwrap();
        assert_ne!(changed, case.expected);
        // Single-body maps have the same bit set. Both reversed rows still
        // satisfy the native LVAL membership check, which is insufficient proof
        // of the independently chosen owned/candidate identities.
        zero::checked_maps(&changed);
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
        );
    }
}

#[test]
fn missing_binding_and_source_qualification_are_distinct_from_zero_pairs() {
    for code in [5, 6] {
        let (case, image, mut stage, requests) = prepared(code);
        stage.long_values.qualified_missing_columns.clear();
        stage.long_values.qualified_zero_columns.push(2);
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
        stage.long_values.qualified_missing_columns.push(2);
        stage.long_values.qualified_zero_columns.clear();
        stage.long_values.initializations[0].binding = DeclarationBinding::ZeroPair {
            offset: 10 * PAGE + case.terminal + 2,
            before: [0; 8],
            after: [0; 8],
        };
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
fn inline_noop_cannot_inherit_an_external_request_tail_extension() {
    for code in [5, 6] {
        let (case, image, mut external, _) = prepared(code);
        let requests = [];
        let (_, mut inline) = staged(&case.before, &requests);
        let unchanged = compose::compose(&image, &inline, &mut budget(), &|| false).unwrap();
        assert_eq!(unchanged, case.before);
        assert!(inline.long_values.qualified_missing_columns.is_empty());
        assert!(inline.long_values.definition_growth.is_none());
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
        inline.long_values.definition_growth = external.long_values.definition_growth.take();
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
