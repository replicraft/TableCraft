use super::*;
use tablecraft_core::values::{Currency, Decimal, ExpressionValue};

fn decimal(coefficient: i128, scale: u8) -> CellValue {
    CellValue::scalar(Value::Decimal(Decimal::new(coefficient, scale).unwrap()))
}

#[test]
fn decimal_equivalent_scales_share_fingerprints_but_keep_original_cells() {
    with_runtime(|runtime| {
        let mut lookup = runtime
            .exact_output_keys(&[crate::OutputType::scalar(ScalarType::Decimal, true)])
            .unwrap()
            .unwrap();
        for values in [
            [(10, 1), (100, 2), (1, 0)],
            [(0, 28), (0, 1), (0, 0)],
            [(-120, 2), (-1200, 3), (-12, 1)],
        ] {
            let mut fingerprint = None;
            for (coefficient, scale) in values {
                let row = [decimal(coefficient, scale)];
                assert!(lookup.valid(&row, runtime.budget, None).unwrap());
                let incoming = lookup.fingerprint(&row, runtime.budget, None).unwrap();
                assert!(incoming.is_some());
                if let Some(fingerprint) = fingerprint {
                    assert_eq!(incoming, Some(fingerprint));
                }
                fingerprint = incoming;
                let ExpressionValue::Value(Value::Decimal(value)) = &row[0].value else {
                    panic!("retained Decimal carrier changed");
                };
                assert_eq!((value.coefficient(), value.scale()), (coefficient, scale));
            }
        }
    });
}

#[test]
fn mixed_fixed_actual_carrier_disables_whole_directory_and_keeps_all_representatives() {
    with_runtime(|runtime| {
        let mut lookup = runtime
            .exact_output_keys(&[crate::OutputType::scalar(ScalarType::Decimal, true)])
            .unwrap();
        let mut retained: Vec<Vec<CellValue>> = Vec::new();
        for (cell, expected) in [
            (decimal(1, 0), None),
            (
                CellValue::scalar(Value::Currency(Currency::from_scaled(20_000))),
                None,
            ),
            (decimal(200, 2), Some(1)),
            (decimal(10, 1), Some(0)),
        ] {
            let row = vec![cell];
            let (representative, fingerprint) = runtime
                .exact_key_match(
                    &mut lookup,
                    &row,
                    &retained,
                    Vec::as_slice,
                    &[TextPolicy::Default],
                )
                .unwrap();
            assert_eq!(representative, expected);
            if representative.is_none() {
                let index = retained.len();
                retained.push(row);
                if let Some(fingerprint) = fingerprint {
                    lookup
                        .as_mut()
                        .unwrap()
                        .insert(fingerprint, index, runtime.budget, None)
                        .unwrap();
                }
            }
        }
        assert!(!lookup.as_ref().unwrap().enabled);
        assert_eq!(retained.len(), 2);
    });
}

#[test]
fn fixed_fingerprint_collisions_still_use_authoritative_numeric_equality() {
    with_runtime(|runtime| {
        let retained = [
            vec![decimal(10, 1)],
            vec![decimal(20, 1)],
            vec![decimal(30, 1)],
        ];
        let mut directory = Directory::default();
        for index in 0..retained.len() {
            directory.insert(0, index, runtime.budget, None).unwrap();
        }
        for (coefficient, expected) in [(100, Some(0)), (200, Some(1)), (400, None)] {
            let row = [decimal(coefficient, 2)];
            let result = directory
                .find(0, |index, same_fingerprint| {
                    assert!(same_fingerprint);
                    runtime.same_expression_row(&row, &retained[index], &[TextPolicy::Default])
                })
                .unwrap();
            assert_eq!(result, expected);
        }
    });
}
