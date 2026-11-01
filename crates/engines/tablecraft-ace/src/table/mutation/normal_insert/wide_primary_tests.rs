//! Direct literal wide-primary proof; no stage, key cache or planner as oracle.
use super::indexed::proof;
use crate::{NativeEditPlan, RowInsert};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
#[ignore = "requires native and SDK-qualified fresh wide-primary literal source images"]
fn literal_wide_primary_images_use_exact_carriers_and_reject_wrong_key_or_history() {
    let dir = PathBuf::from(std::env::var_os("TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR").unwrap());
    let old = PathBuf::from(std::env::var_os("TABLECRAFT_INTEGRAL_EXTRA_ORIGINALS_DIR").unwrap());
    assert!(dir.is_absolute() && old.is_absolute());
    let mut literals = Vec::new();
    for (header, table, recipe, td, data, root, filename, hash) in [
        (
            5_u8,
            "Int64WidePrimary",
            "insert-null",
            103_u32,
            108_u32,
            105_u32,
            "h5-Int64WidePrimary-insert-null.accdb",
            "1170326fdc67517f60dc35719c4b8d3ee47d34aa00b0e01aa21c8faceebfc0a6",
        ),
        (
            5_u8,
            "Int64WidePrimary",
            "insert-zero",
            103_u32,
            108_u32,
            105_u32,
            "h5-Int64WidePrimary-insert-zero.accdb",
            "8f67ba4ea2f53174e936ca60c0121ddedff1bccbc2cd83db6d3783e7edc481a0",
        ),
        (
            5_u8,
            "CurrencyWidePrimary",
            "insert-null",
            110_u32,
            115_u32,
            112_u32,
            "h5-CurrencyWidePrimary-insert-null.accdb",
            "70b6544bdb77bd1f5bfd7808c28f66b6bba2c281a31d73aced09cba54e5dc565",
        ),
        (
            5_u8,
            "CurrencyWidePrimary",
            "insert-zero",
            110_u32,
            115_u32,
            112_u32,
            "h5-CurrencyWidePrimary-insert-zero.accdb",
            "5b419c61c89ffd52f9166c39bbc9c58b8cdc3e4915998fae8ce9cafb9bcf77a0",
        ),
        (
            6_u8,
            "Int64WidePrimary",
            "insert-null",
            120_u32,
            126_u32,
            122_u32,
            "h6-Int64WidePrimary-insert-null.accdb",
            "7b19ebe1be715123235e0b6e64fbfa86798843a19229dce40980756dfd3926a9",
        ),
        (
            6_u8,
            "Int64WidePrimary",
            "insert-zero",
            120_u32,
            126_u32,
            122_u32,
            "h6-Int64WidePrimary-insert-zero.accdb",
            "b10032b46b0e153c2b5541861273b4839815b882f79ebc4277a164cbda1b756c",
        ),
        (
            6_u8,
            "CurrencyWidePrimary",
            "insert-null",
            128_u32,
            133_u32,
            130_u32,
            "h6-CurrencyWidePrimary-insert-null.accdb",
            "3fdd0540be9147a3a5d0353194f1f37ff29b7e5351b31ccf1af79bd50cd625e9",
        ),
        (
            6_u8,
            "CurrencyWidePrimary",
            "insert-zero",
            128_u32,
            133_u32,
            130_u32,
            "h6-CurrencyWidePrimary-insert-zero.accdb",
            "c90921bd6afa891d7152403214b814ba749ce6183c1424ad539f2ecdf3fe2e19",
        ),
    ] {
        let before =
            fs::read(old.join(format!("h{header}-integral-extra-original.accdb"))).unwrap();
        assert_eq!(
            digest(&before),
            if header == 5 {
                "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e"
            } else {
                "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7"
            }
        );
        let after = fs::read(dir.join(filename)).unwrap();
        assert_eq!(digest(&after), hash);
        literals.push((table, recipe, td, data, root, before, after));
    }
    assert_eq!(literals.len(), 8);
    for (table, recipe, td, data, root, before, after) in literals {
        let scalar = |units| {
            if table.starts_with("Currency") {
                Value::Currency(Currency::from_scaled(units))
            } else {
                Value::Int64(units)
            }
        };
        let original = vec![
            scalar(1),
            if recipe == "insert-null" {
                Value::Null
            } else {
                scalar(0)
            },
            Value::Int32(68),
        ];
        let requests = vec![RowInsert::new(original.clone())];
        let plan = NativeEditPlan {
            before: before.clone(),
            after: after.clone(),
            changed_rows: 1,
        };
        let scope = CatalogScope::new(200);
        proof::prove(&plan, scope, table, &requests, &mut budget(), &|| false).unwrap();
        for at in [
            root as usize * 4096 + 480,
            td as usize * 4096 + 67,
            data as usize * 4096 + 4006,
            24,
        ] {
            let mut forged = after.clone();
            forged[at] ^= 1;
            let plan = NativeEditPlan {
                before: before.clone(),
                after: forged,
                changed_rows: 1,
            };
            assert!(
                proof::prove(&plan, scope, table, &requests, &mut budget(), &|| false).is_err()
            );
        }
        let count = NativeEditPlan {
            before: before.clone(),
            after: after.clone(),
            changed_rows: 0,
        };
        assert!(proof::prove(&count, scope, table, &requests, &mut budget(), &|| false).is_err());
        let mut wrong = original;
        wrong[0] = if table.starts_with("Currency") {
            Value::Int64(1)
        } else {
            Value::Currency(Currency::from_scaled(1))
        };
        assert!(
            proof::prove(
                &plan,
                scope,
                table,
                &[RowInsert::new(wrong)],
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
}
