//! Source-qualified exact wide-primary normal action admission; legacy qualifier remains separate.
use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_ace::{
    EditError, IndexConstraintViolation, InspectError, NativeDeleteIntent, NativeImage,
    NativeInsertIntent, RecoverableEditError, RowDelete, RowInsert, WriteBlocker,
    qualify_native_delete_recovery, qualify_native_insert_recovery,
};
use tablecraft_core::values::{Currency, Value};
use tablecraft_io::journal::{RecoveryOutcome, inspect_v1};

struct Case {
    table: &'static str,
    family: &'static str,
    recipe: &'static str,
    data: u32,
    before: Vec<u8>,
    after: Vec<u8>,
}

fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for (env, table, family, recipe, data, before, oldhash, after, newhash) in [
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "Int64WidePrimary",
            "wide-primary",
            "insert-null",
            108_u32,
            "h5-Int64WidePrimary-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
            "h5-Int64WidePrimary-insert-null.accdb",
            "1170326fdc67517f60dc35719c4b8d3ee47d34aa00b0e01aa21c8faceebfc0a6",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "Int64WidePrimary",
            "wide-primary",
            "insert-zero",
            108_u32,
            "h5-Int64WidePrimary-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
            "h5-Int64WidePrimary-insert-zero.accdb",
            "8f67ba4ea2f53174e936ca60c0121ddedff1bccbc2cd83db6d3783e7edc481a0",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "Int64WidePrimary",
            "wide-primary",
            "delete-all",
            108_u32,
            "h5-Int64WidePrimary-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
            "h5-Int64WidePrimary-delete-all.accdb",
            "ac32cb6bfe4601fe8695584c8363b5ef6b9a30253ae7cd32702cd26397a671b3",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "CurrencyWidePrimary",
            "wide-primary",
            "insert-null",
            115_u32,
            "h5-CurrencyWidePrimary-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
            "h5-CurrencyWidePrimary-insert-null.accdb",
            "70b6544bdb77bd1f5bfd7808c28f66b6bba2c281a31d73aced09cba54e5dc565",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "CurrencyWidePrimary",
            "wide-primary",
            "insert-zero",
            115_u32,
            "h5-CurrencyWidePrimary-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
            "h5-CurrencyWidePrimary-insert-zero.accdb",
            "5b419c61c89ffd52f9166c39bbc9c58b8cdc3e4915998fae8ce9cafb9bcf77a0",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "CurrencyWidePrimary",
            "wide-primary",
            "delete-all",
            115_u32,
            "h5-CurrencyWidePrimary-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
            "h5-CurrencyWidePrimary-delete-all.accdb",
            "830586432a31c26c3d687d7287750af5a104c5c9d8a99fc415805f7b250c0293",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "Int64WidePrimary",
            "wide-primary",
            "insert-null",
            126_u32,
            "h6-Int64WidePrimary-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
            "h6-Int64WidePrimary-insert-null.accdb",
            "7b19ebe1be715123235e0b6e64fbfa86798843a19229dce40980756dfd3926a9",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "Int64WidePrimary",
            "wide-primary",
            "insert-zero",
            126_u32,
            "h6-Int64WidePrimary-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
            "h6-Int64WidePrimary-insert-zero.accdb",
            "b10032b46b0e153c2b5541861273b4839815b882f79ebc4277a164cbda1b756c",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "Int64WidePrimary",
            "wide-primary",
            "delete-all",
            126_u32,
            "h6-Int64WidePrimary-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
            "h6-Int64WidePrimary-delete-all.accdb",
            "2e7bb9c3d829b61eb28c3b9aad9d6015ed2616bf90809db182c6b34fb2d64240",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "CurrencyWidePrimary",
            "wide-primary",
            "insert-null",
            133_u32,
            "h6-CurrencyWidePrimary-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
            "h6-CurrencyWidePrimary-insert-null.accdb",
            "3fdd0540be9147a3a5d0353194f1f37ff29b7e5351b31ccf1af79bd50cd625e9",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "CurrencyWidePrimary",
            "wide-primary",
            "insert-zero",
            133_u32,
            "h6-CurrencyWidePrimary-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
            "h6-CurrencyWidePrimary-insert-zero.accdb",
            "c90921bd6afa891d7152403214b814ba749ce6183c1424ad539f2ecdf3fe2e19",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "CurrencyWidePrimary",
            "wide-primary",
            "delete-all",
            133_u32,
            "h6-CurrencyWidePrimary-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
            "h6-CurrencyWidePrimary-delete-all.accdb",
            "cf9df1d97646745e0e12116f825c377f771835f1ac20a7d6c3e5129bee11899a",
        ),
    ] {
        let dir = PathBuf::from(std::env::var_os(env).unwrap());
        assert!(dir.is_absolute());
        let before = fs::read(dir.join(before)).unwrap();
        let after = fs::read(dir.join(after)).unwrap();
        assert_eq!(digest(&before), oldhash);
        assert_eq!(digest(&after), newhash);
        cases.push(Case {
            table,
            family,
            recipe,
            data,
            before,
            after,
        });
    }
    assert_eq!(cases.len(), 12);
    cases
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn request(case: &Case) -> Vec<Value> {
    let key = if case.recipe == "insert-null" {
        Value::Null
    } else if case.table.starts_with("Currency") {
        Value::Currency(Currency::from_scaled(0))
    } else {
        Value::Int64(0)
    };
    vec![
        if case.table.starts_with("Currency") {
            Value::Currency(Currency::from_scaled(1))
        } else {
            Value::Int64(1)
        },
        key,
        Value::Int32(68),
    ]
}

fn deletes(case: &Case) -> Vec<RowDelete> {
    assert_ne!(case.family, "empty");
    (0..3)
        .map(|slot| {
            let start = case.data as usize * 4096 + 4073 - slot * 23;
            RowDelete::new(slot, case.before[start..start + 23].to_vec())
        })
        .collect()
}

fn wire(case: &Case) -> Vec<u8> {
    let insert = case.recipe.starts_with("insert-");
    let mut body = Vec::new();
    let count = if insert { 1 } else { 3 };
    if insert {
        let mut payload = 3_u32.to_le_bytes().to_vec();
        for value in request(case) {
            let (tag, raw) = match value {
                Value::Null => (0, Vec::new()),
                Value::Int32(value) => (2, value.to_le_bytes().to_vec()),
                Value::Int64(value) => (3, value.to_le_bytes().to_vec()),
                Value::Currency(value) => (4, value.scaled().to_le_bytes().to_vec()),
                _ => unreachable!(),
            };
            payload.extend_from_slice(&[tag, 0, 0, 0]);
            payload.extend_from_slice(&(raw.len() as u32).to_le_bytes());
            payload.extend_from_slice(&raw);
        }
        body.extend_from_slice(&((payload.len() + 4) as u32).to_le_bytes());
        body.extend_from_slice(&payload);
    } else {
        for row in deletes(case) {
            body.extend_from_slice(&(row.row() as u64).to_le_bytes());
            body.extend_from_slice(&23_u32.to_le_bytes());
            body.extend_from_slice(row.expected_bytes());
        }
    }
    let length = 68 + case.table.len() + body.len();
    let mut raw = if insert {
        Vec::from(*b"TCNA0001")
    } else {
        Vec::from(*b"TCND0001")
    };
    raw.extend_from_slice(&(length as u32).to_le_bytes());
    raw.extend_from_slice(&[1, 0, 0, 0]);
    raw.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    raw.extend_from_slice(&digest.finalize());
    for value in [case.table.len() as u32, count, 0] {
        raw.extend_from_slice(&value.to_le_bytes());
    }
    raw.extend_from_slice(case.table.as_bytes());
    raw.extend_from_slice(&body);
    assert_eq!(raw.len(), length);
    raw
}

#[test]
#[ignore = "requires48 native/SDK-qualified fresh numeric source views"]
fn wide_primary_actions_retain_exact_original_carriers_histories_and_wires() {
    for case in cases() {
        let mut target = Storage::new(case.before.clone());
        let mut journal = Storage::new(Vec::new());
        if case.recipe.starts_with("insert-") {
            let original = request(&case);
            let plan = image(&case.before)
                .plan_recoverable_ordinary_row_inserts(
                    case.table,
                    vec![RowInsert::new(original.clone())],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.after_bytes(), case.after);
            assert_eq!(plan.inserted_rows(), 1);
            assert_eq!(plan.intent().rows()[0].values(), original);
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                wire(&case)
            );
            assert_eq!(
                plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap()
                    .commit(&mut budget(), &|| false)
                    .unwrap(),
                1
            );
        } else {
            let plan = image(&case.before)
                .plan_recoverable_ordinary_row_deletes(
                    case.table,
                    deletes(&case),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.after_bytes(), case.after);
            assert_eq!(plan.deleted_rows(), 3);
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                wire(&case)
            );
            assert_eq!(
                plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap()
                    .commit(&mut budget(), &|| false)
                    .unwrap(),
                3
            );
        }
        assert_eq!(target.bytes.get_ref(), &case.after);
        let empty_insert = image(&case.before)
            .plan_recoverable_ordinary_row_inserts(case.table, Vec::new(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(empty_insert.inserted_rows(), 0);
        assert_eq!(empty_insert.after_bytes(), case.before);
        let empty_delete = image(&case.before)
            .plan_recoverable_ordinary_row_deletes(case.table, Vec::new(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(empty_delete.deleted_rows(), 0);
        assert_eq!(empty_delete.after_bytes(), case.before);
    }
}

#[test]
#[ignore = "requires48 native/SDK-qualified fresh numeric source views"]
fn wide_primary_actions_recover_original_images_and_primary_history() {
    for case in cases()
        .into_iter()
        .filter(|case| case.recipe == "insert-zero")
    {
        let original = wire(&case);
        for marker in [Marker::Pending, Marker::Committed] {
            let mut target = Storage::new(case.after.clone());
            let mut journal = Storage::new(journals::journal(&case.before, &case.after, marker));
            let committed = matches!(marker, Marker::Committed);
            for _ in 0..2 {
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let intent =
                    NativeInsertIntent::decode(&original, &mut budget(), &|| false).unwrap();
                let actual = qualify_native_insert_recovery(
                    checked,
                    intent,
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap()
                .recover(&mut target, &mut budget(), &|| false)
                .unwrap();
                assert_eq!(
                    actual,
                    if committed {
                        RecoveryOutcome::Committed
                    } else {
                        RecoveryOutcome::RolledBack
                    }
                );
                assert_eq!(
                    target.bytes.get_ref(),
                    if committed { &case.after } else { &case.before }
                );
            }
            let writes = target.writes;
            target.fail_prefix = Some(0);
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let intent = NativeInsertIntent::decode(&original, &mut budget(), &|| false).unwrap();
            qualify_native_insert_recovery(checked, intent, scope(), &mut budget(), &|| false)
                .unwrap()
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(target.writes, writes);
        }
    }
    // DELETE recovery remains separately exercised from the same literal corpus.
    for case in cases()
        .into_iter()
        .filter(|case| case.recipe == "delete-all" && case.family == "wide-primary")
    {
        let mut target = Storage::new(case.after.clone());
        let mut journal = Storage::new(journals::journal(
            &case.before,
            &case.after,
            Marker::Pending,
        ));
        let intent = NativeDeleteIntent::decode(&wire(&case), &mut budget(), &|| false).unwrap();
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        qualify_native_delete_recovery(checked, intent, scope(), &mut budget(), &|| false)
            .unwrap()
            .recover(&mut target, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(target.bytes.get_ref(), &case.before);
    }
}

#[test]
#[ignore = "requires48 native/SDK-qualified fresh numeric source views"]
fn wide_primary_constraints_and_exact_carrier_identity_still_refuse_before_journal() {
    for case in cases()
        .into_iter()
        .filter(|case| case.recipe == "insert-zero")
    {
        let minimum = if case.table.starts_with("Currency") {
            Value::Currency(Currency::from_scaled(i64::MIN))
        } else {
            Value::Int64(i64::MIN)
        };
        for (id, wanted) in [
            (minimum, IndexConstraintViolation::DuplicateKey),
            (Value::Null, IndexConstraintViolation::NullKey),
        ] {
            let mut row = request(&case);
            row[0] = id;
            let error = image(&case.before)
                .plan_recoverable_ordinary_row_inserts(
                    case.table,
                    vec![RowInsert::new(row)],
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap();
            assert!(
                matches!(error, RecoverableEditError::Edit(EditError::Inspect(InspectError::IndexConstraint(actual))) if actual == wanted)
            );
        }
        let mut row = request(&case);
        row[0] = if case.table.starts_with("Currency") {
            Value::Int64(1)
        } else {
            Value::Currency(Currency::from_scaled(1))
        };
        let error = image(&case.before)
            .plan_recoverable_ordinary_row_inserts(
                case.table,
                vec![RowInsert::new(row)],
                &mut budget(),
                &|| false,
            )
            .err()
            .unwrap();
        assert!(matches!(
            error,
            RecoverableEditError::Edit(EditError::Unsupported(WriteBlocker::IndexedColumn))
        ));
    }
}
