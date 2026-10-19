#![cfg(feature = "ace16-write-preview")]

#[path = "mutation/fixture.rs"]
mod fixture;
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
#[path = "composite_indexes/fixture.rs"]
mod numeric_fixture;
#[path = "ace16/scalar_fixture.rs"]
mod scalar_fixture;
mod support;

use fixture as base;
use proptest::prelude::*;
use std::io::Cursor;
use tablecraft_ace::{EditError, FixedScalarUpdate, NativeImage, WriteBlocker, read_table};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Double, Guid, LegacyDateTime, Single, Value},
};
use tablecraft_model::CatalogScope;

struct Case {
    kind: u8,
    before: Value,
    before_bytes: Vec<u8>,
    after: Value,
    after_bytes: Vec<u8>,
}
fn cases() -> Vec<Case> {
    vec![
        Case {
            kind: 2,
            before: Value::Byte(1),
            before_bytes: vec![1],
            after: Value::Byte(255),
            after_bytes: vec![255],
        },
        Case {
            kind: 3,
            before: Value::Int16(i16::MIN),
            before_bytes: vec![0, 0x80],
            after: Value::Int16(i16::MAX),
            after_bytes: vec![0xff, 0x7f],
        },
        Case {
            kind: 4,
            before: Value::Int32(i32::MIN),
            before_bytes: vec![0, 0, 0, 0x80],
            after: Value::Int32(i32::MAX),
            after_bytes: vec![0xff, 0xff, 0xff, 0x7f],
        },
        Case {
            kind: 0x13,
            before: Value::Int64(i64::MIN),
            before_bytes: vec![0, 0, 0, 0, 0, 0, 0, 0x80],
            after: Value::Int64(i64::MAX),
            after_bytes: vec![0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f],
        },
        Case {
            kind: 5,
            before: Value::Currency(Currency::from_scaled(-1)),
            before_bytes: vec![0xff; 8],
            after: Value::Currency(Currency::from_scaled(1)),
            after_bytes: vec![1, 0, 0, 0, 0, 0, 0, 0],
        },
        Case {
            kind: 6,
            before: Value::Single(Single::from_bits(0x80000000)),
            before_bytes: vec![0, 0, 0, 0x80],
            after: Value::Single(Single::from_bits(0x7fc00001)),
            after_bytes: vec![1, 0, 0xc0, 0x7f],
        },
        Case {
            kind: 7,
            before: Value::Double(Double::from_bits(0x8000000000000000)),
            before_bytes: vec![0, 0, 0, 0, 0, 0, 0, 0x80],
            after: Value::Double(Double::from_bits(0x7ff8000000000001)),
            after_bytes: vec![1, 0, 0, 0, 0, 0, 0xf8, 0x7f],
        },
        Case {
            kind: 8,
            before: Value::DateTime(LegacyDateTime::from_bits(0xbff4000000000000).unwrap()),
            before_bytes: vec![0, 0, 0, 0, 0, 0, 0xf4, 0xbf],
            after: Value::DateTime(LegacyDateTime::from_bits(0x8000000000000000).unwrap()),
            after_bytes: vec![0, 0, 0, 0, 0, 0, 0, 0x80],
        },
        Case {
            kind: 15,
            before: Value::Guid(Guid::from_bytes([0; 16])),
            before_bytes: vec![0; 16],
            after: Value::Guid(Guid::from_bytes([
                0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
                0xee, 0xff,
            ])),
            after_bytes: vec![
                0x33, 0x22, 0x11, 0, 0x55, 0x44, 0x77, 0x66, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
                0xee, 0xff,
            ],
        },
    ]
}
fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 10_000_000)
}
fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn exact_carrier_goldens_preserve_every_other_byte_and_reopen_without_coercion() {
    for code in [5, 6] {
        for case in cases() {
            let bytes = scalar_fixture::fixed_scalar(
                code,
                case.kind,
                case.before_bytes.len(),
                &[Some(case.before_bytes.clone())],
            );
            let preview = capture(&bytes)
                .preview_ace16_fixed_updates(
                    "Items",
                    "Score",
                    &[FixedScalarUpdate::new(
                        0,
                        case.before.clone(),
                        case.after.clone(),
                    )],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            let mut expected = bytes.clone();
            let row = u16::from_le_bytes(bytes[12 * 4096 + 14..12 * 4096 + 16].try_into().unwrap())
                as usize
                & 0x1fff;
            expected[12 * 4096 + row + 6..12 * 4096 + row + 6 + case.after_bytes.len()]
                .copy_from_slice(&case.after_bytes);
            assert_eq!(preview.before_bytes(), bytes);
            assert_eq!(preview.candidate_bytes(), expected);
            assert_eq!(preview.changed_rows(), 1);
            let rows = read_table(
                &mut Cursor::new(preview.candidate_bytes()),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(rows.rows()[0][1], case.after);
            let no_op = capture(&bytes)
                .preview_ace16_fixed_updates(
                    "Items",
                    "Score",
                    &[FixedScalarUpdate::new(0, case.before.clone(), case.before)],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(no_op.changed_rows(), 0);
            assert_eq!(no_op.candidate_bytes(), bytes);
        }
    }
}

#[test]
fn wrong_stale_and_duplicate_carriers_reject_and_never_change_the_source() {
    for case in cases() {
        let bytes = scalar_fixture::fixed_scalar(
            6,
            case.kind,
            case.before_bytes.len(),
            &[Some(case.before_bytes)],
        );
        for (expected, value) in [
            (Value::Text("1".into()), case.after.clone()),
            (Value::Binary(vec![]), case.after.clone()),
        ] {
            assert_eq!(
                capture(&bytes)
                    .preview_ace16_fixed_updates(
                        "Items",
                        "Score",
                        &[FixedScalarUpdate::new(0, expected, value)],
                        &mut budget(),
                        &|| false
                    )
                    .err(),
                Some(EditError::Unsupported(WriteBlocker::Column))
            );
        }
        assert_eq!(
            capture(&bytes)
                .preview_ace16_fixed_updates(
                    "Items",
                    "Score",
                    &[FixedScalarUpdate::new(
                        0,
                        case.after.clone(),
                        case.before.clone()
                    )],
                    &mut budget(),
                    &|| false
                )
                .err(),
            Some(EditError::Conflict)
        );
        let duplicate = [
            FixedScalarUpdate::new(0, case.before.clone(), case.after.clone()),
            FixedScalarUpdate::new(0, case.before, case.after),
        ];
        assert_eq!(
            capture(&bytes)
                .preview_ace16_fixed_updates("Items", "Score", &duplicate, &mut budget(), &|| false)
                .err(),
            Some(EditError::Conflict)
        );
    }
    let bytes = scalar_fixture::fixed_scalar(6, 0x13, 8, &[None]);
    assert_eq!(
        capture(&bytes)
            .preview_ace16_fixed_updates(
                "Items",
                "Score",
                &[FixedScalarUpdate::new(0, Value::Int64(0), Value::Int64(1))],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Conflict)
    );
}

#[test]
fn small_budget_cancellation_unsupported_masks_and_column_flags_still_block() {
    let bytes = scalar_fixture::fixed_scalar(6, 0x13, 8, &[Some(vec![0; 8])]);
    for (mut allowance, cancelled) in [(budget(), true), (ResourceBudget::new(0, 0, 0), false)] {
        assert!(
            capture(&bytes)
                .preview_ace16_fixed_updates("Items", "Score", &[], &mut allowance, &|| cancelled)
                .is_err()
        );
    }
    for flags in [0x02, 0x07, 0x13] {
        let mut changed = bytes.clone();
        changed[10 * 4096 + 63 + 25 + 15] = flags;
        assert!(
            capture(&changed)
                .preview_ace16_fixed_updates("Items", "Score", &[], &mut budget(), &|| false)
                .is_err()
        );
    }
    let boolean = scalar_fixture::fixed_scalar(6, 1, 0, &[Some(vec![])]);
    assert_eq!(
        capture(&boolean)
            .preview_ace16_fixed_updates("Items", "Score", &[], &mut budget(), &|| false)
            .unwrap()
            .changed_rows(),
        0
    );
}

#[test]
fn indexed_non_int32_targets_remain_explicitly_unsupported_even_for_noops() {
    let bytes =
        numeric_fixture::database(6, &[4, 0x13], &[(1, false)], &[vec![Some(1), Some(5)]], 0);
    for updates in [
        vec![],
        vec![FixedScalarUpdate::new(0, Value::Int64(5), Value::Int64(5))],
        vec![FixedScalarUpdate::new(0, Value::Int64(5), Value::Int64(6))],
    ] {
        assert_eq!(
            capture(&bytes)
                .preview_ace16_fixed_updates("Items", "Score", &updates, &mut budget(), &|| false)
                .err(),
            Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
        );
    }
    for flags in [0x82, 0x90] {
        let mut changed = bytes.clone();
        numeric_fixture::index_flags(&mut changed, flags);
        assert_eq!(
            capture(&changed)
                .preview_ace16_fixed_updates("Items", "Score", &[], &mut budget(), &|| false)
                .err(),
            Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
        );
    }
}

#[test]
fn typed_int32_candidates_match_existing_indexed_leaf_and_node_contracts() {
    for mut bytes in [
        index_fixture::database(false, true, false),
        index_fixture::database(true, false, false),
        index_fixture::node_database(),
    ] {
        bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
        bytes[20] = 6;
        let narrow = capture(&bytes)
            .preview_ace16_int32_updates(
                "Items",
                "Score",
                &[tablecraft_ace::Int32Update::new(1, 20, 35)],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let typed = capture(&bytes)
            .preview_ace16_fixed_updates(
                "Items",
                "Score",
                &[FixedScalarUpdate::new(
                    1,
                    Value::Int32(20),
                    Value::Int32(35),
                )],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(typed.candidate_bytes(), narrow.candidate_bytes());
        assert_eq!(typed.changed_rows(), narrow.changed_rows());
    }
}

proptest! {
    #![proptest_config(ProptestConfig{cases:128,rng_seed:proptest::test_runner::RngSeed::Fixed(0x4649584544504154),..Default::default()})]
    #[test]
    fn wide_multirow_changes_match_an_independent_model(values in prop::collection::vec(any::<i64>(),0..32)) {
        let input=values.iter().map(|value|Some(value.to_le_bytes().to_vec())).collect::<Vec<_>>();
        let bytes=scalar_fixture::fixed_scalar(6,0x13,8,&input);
        let updates=values.iter().enumerate().filter(|(i,_)|i%2==0).map(|(i,v)|FixedScalarUpdate::new(i,Value::Int64(*v),Value::Int64(!v))).collect::<Vec<_>>();
        let preview=capture(&bytes).preview_ace16_fixed_updates("Items","Score",&updates,&mut budget(),&||false).unwrap();
        prop_assert_eq!(preview.changed_rows(),updates.len());
        let rows=read_table(&mut Cursor::new(preview.candidate_bytes()),CatalogScope::new(1),"Items",&mut budget(),&||false).unwrap();
        for (i,(row,original)) in rows.rows().iter().zip(values).enumerate() {
            prop_assert_eq!(&row[0],&Value::Int32(i as i32+1));
            prop_assert_eq!(&row[1],&Value::Int64(if i%2==0 {!original}else{original}));
        }
    }
}
