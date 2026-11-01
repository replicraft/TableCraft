//! Literal dense empty/all-NULL follow-ons; no fresh mutation images as oracle.
use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_ace::{
    NativeDeleteIntent, NativeImage, NativeInsertIntent, RowDelete, RowInsert,
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
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            "Int64Keys",
            "empty",
            "insert-null",
            94_u32,
            "h5-Int64Keys-empty-before.accdb",
            "c73a65b461f0d52a73ec47aeba77e40ecf1d2c6686744360d4ca977049690dc8",
            "h5-Int64Keys-empty-insert-null.accdb",
            "790535e87bbc1823ce0424673911be29c6d3c8873d19553672da391c980b375a",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            "Int64Keys",
            "empty",
            "insert-zero",
            94_u32,
            "h5-Int64Keys-empty-before.accdb",
            "c73a65b461f0d52a73ec47aeba77e40ecf1d2c6686744360d4ca977049690dc8",
            "h5-Int64Keys-empty-insert-zero.accdb",
            "c1e17f6ac9665e9e461537b4704bc8af9ac1dd6dfc3bcceeebcc786bd518d17e",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            "Int64Keys",
            "all-null",
            "insert-null",
            94_u32,
            "h5-Int64Keys-all-null-before.accdb",
            "63f2a5233dd6c0d4ec77472f57174264e6536193ab90265b335a84bfb1a73162",
            "h5-Int64Keys-all-null-insert-null.accdb",
            "6b280b1f45195e6d4444a72c6a4ed31ee71e0623a862eae55165456aab0cb4cd",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            "Int64Keys",
            "all-null",
            "insert-zero",
            94_u32,
            "h5-Int64Keys-all-null-before.accdb",
            "63f2a5233dd6c0d4ec77472f57174264e6536193ab90265b335a84bfb1a73162",
            "h5-Int64Keys-all-null-insert-zero.accdb",
            "1db6655504e733a1322178f0c1448634643e83b939d541f84d026c88101d4632",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            "Int64Keys",
            "all-null",
            "delete-all",
            94_u32,
            "h5-Int64Keys-all-null-before.accdb",
            "63f2a5233dd6c0d4ec77472f57174264e6536193ab90265b335a84bfb1a73162",
            "h5-Int64Keys-all-null-delete-all.accdb",
            "8e92fda8b4b35d22399c7d705d969c5e80594fcf8d897b1830712036f2526cbd",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            "CurrencyKeys",
            "empty",
            "insert-null",
            101_u32,
            "h5-CurrencyKeys-empty-before.accdb",
            "6dbfe1d18755164a42b0b6567d13cae9e2183cd51ff12b65614b98071a83ccf3",
            "h5-CurrencyKeys-empty-insert-null.accdb",
            "cbc31666db762f062c305f9fae1e9306bfadd90e74d41a57a65a9cad456c84c1",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            "CurrencyKeys",
            "empty",
            "insert-zero",
            101_u32,
            "h5-CurrencyKeys-empty-before.accdb",
            "6dbfe1d18755164a42b0b6567d13cae9e2183cd51ff12b65614b98071a83ccf3",
            "h5-CurrencyKeys-empty-insert-zero.accdb",
            "28ca3c43882d8b3650b89d8eef1b090ca5d3d661a86d37b6c3d7053eb58f2380",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            "CurrencyKeys",
            "all-null",
            "insert-null",
            101_u32,
            "h5-CurrencyKeys-all-null-before.accdb",
            "2b1b112c32e211b570f39a5ed7eb479e7fb4fea013c79ba0e51863480bdf8c0f",
            "h5-CurrencyKeys-all-null-insert-null.accdb",
            "ec164c3c7d8f293f535169b9da6cfe776ae40553952c9fe4ac891203fd68519f",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            "CurrencyKeys",
            "all-null",
            "insert-zero",
            101_u32,
            "h5-CurrencyKeys-all-null-before.accdb",
            "2b1b112c32e211b570f39a5ed7eb479e7fb4fea013c79ba0e51863480bdf8c0f",
            "h5-CurrencyKeys-all-null-insert-zero.accdb",
            "c29178aba34d6068b02acd728e1e336b26cf2aa6954199b9d42f5cbffe0f8841",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            "CurrencyKeys",
            "all-null",
            "delete-all",
            101_u32,
            "h5-CurrencyKeys-all-null-before.accdb",
            "2b1b112c32e211b570f39a5ed7eb479e7fb4fea013c79ba0e51863480bdf8c0f",
            "h5-CurrencyKeys-all-null-delete-all.accdb",
            "40e423e197bdf0976387b89420ee1052c05a66b73892c4cf356dbe0bb4d31317",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            "Int64Keys",
            "empty",
            "insert-null",
            111_u32,
            "h6-Int64Keys-empty-before.accdb",
            "97ec82868ed17533051bb0345864b642a44951d45b41fcd9ffffc61437589a0b",
            "h6-Int64Keys-empty-insert-null.accdb",
            "d9b34465d75eda6ac4dfadae0f5f719e95b11d41879690e176f10ef6bcee78ba",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            "Int64Keys",
            "empty",
            "insert-zero",
            111_u32,
            "h6-Int64Keys-empty-before.accdb",
            "97ec82868ed17533051bb0345864b642a44951d45b41fcd9ffffc61437589a0b",
            "h6-Int64Keys-empty-insert-zero.accdb",
            "4996b9f0df9201972e2867d02e97ca6abb4fd9be3d8c3864d7e61b58fa61972e",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            "Int64Keys",
            "all-null",
            "insert-null",
            111_u32,
            "h6-Int64Keys-all-null-before.accdb",
            "c09cd358566950b85c3b7cccb207d4b54831551f37aa43cbc6a7841d57333bef",
            "h6-Int64Keys-all-null-insert-null.accdb",
            "994ebcd2c00ce0e81d870f5db386e56d63e872741ef0b8690d696006d9cf71f2",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            "Int64Keys",
            "all-null",
            "insert-zero",
            111_u32,
            "h6-Int64Keys-all-null-before.accdb",
            "c09cd358566950b85c3b7cccb207d4b54831551f37aa43cbc6a7841d57333bef",
            "h6-Int64Keys-all-null-insert-zero.accdb",
            "969f0b7ab6e132287215ee819e09bd6c4d7e53fa39312fd6ab479d94c4a2694e",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            "Int64Keys",
            "all-null",
            "delete-all",
            111_u32,
            "h6-Int64Keys-all-null-before.accdb",
            "c09cd358566950b85c3b7cccb207d4b54831551f37aa43cbc6a7841d57333bef",
            "h6-Int64Keys-all-null-delete-all.accdb",
            "bc64bd53f6aab1cf12c3b5754221f065567df7dce4a0ecd702c54b0af9580087",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            "CurrencyKeys",
            "empty",
            "insert-null",
            118_u32,
            "h6-CurrencyKeys-empty-before.accdb",
            "9600a6e1449f28485af22006de9730f9c4ff013d712a0606c38715fc1ac9a35a",
            "h6-CurrencyKeys-empty-insert-null.accdb",
            "4a65d29c66d4c63921006fc2051a9fe5cf6a88203b8e94215da85bbd888356e5",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            "CurrencyKeys",
            "empty",
            "insert-zero",
            118_u32,
            "h6-CurrencyKeys-empty-before.accdb",
            "9600a6e1449f28485af22006de9730f9c4ff013d712a0606c38715fc1ac9a35a",
            "h6-CurrencyKeys-empty-insert-zero.accdb",
            "6703ed09a5d6ee3ba5c578e0741873fe4dc0555c8a66f43b192b4e40a936bb28",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            "CurrencyKeys",
            "all-null",
            "insert-null",
            118_u32,
            "h6-CurrencyKeys-all-null-before.accdb",
            "3b22521e8ee606bd5e3fe6203500655af57e8dda73a4424825f74cfbd09705f2",
            "h6-CurrencyKeys-all-null-insert-null.accdb",
            "ef60d39985748d91a9f1b0288016b906c6cc698000ff3fad5fda3b276efff6a3",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            "CurrencyKeys",
            "all-null",
            "insert-zero",
            118_u32,
            "h6-CurrencyKeys-all-null-before.accdb",
            "3b22521e8ee606bd5e3fe6203500655af57e8dda73a4424825f74cfbd09705f2",
            "h6-CurrencyKeys-all-null-insert-zero.accdb",
            "9d61f894a55d71c3cbb59ee850ef049b893a1fa73cca13644d62fa16739bb7a9",
        ),
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            "CurrencyKeys",
            "all-null",
            "delete-all",
            118_u32,
            "h6-CurrencyKeys-all-null-before.accdb",
            "3b22521e8ee606bd5e3fe6203500655af57e8dda73a4424825f74cfbd09705f2",
            "h6-CurrencyKeys-all-null-delete-all.accdb",
            "64e0b24ed4c428598b9deba225179831232f33f90a513b5fc5645ee6493fac79",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "Int64AllNull",
            "fresh-all-null",
            "insert-null",
            94_u32,
            "h5-Int64AllNull-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
            "h5-Int64AllNull-insert-null.accdb",
            "bdde173f66105ca697130e401fd8a498c3f5a04bf2f367d4661ceb66453cb911",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "Int64AllNull",
            "fresh-all-null",
            "insert-zero",
            94_u32,
            "h5-Int64AllNull-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
            "h5-Int64AllNull-insert-zero.accdb",
            "d20a34d871037da0492a86363c9de34225c49508f269a3f0d9a04fc02895e536",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "Int64AllNull",
            "fresh-all-null",
            "delete-all",
            94_u32,
            "h5-Int64AllNull-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
            "h5-Int64AllNull-delete-all.accdb",
            "6862fecceefbf171d73535b78e88e3ed86692c3f059ce6f7690d1d70deea1c65",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "CurrencyAllNull",
            "fresh-all-null",
            "insert-null",
            101_u32,
            "h5-CurrencyAllNull-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
            "h5-CurrencyAllNull-insert-null.accdb",
            "f0d96ecf58ddf6dcfef9da8ab9040bd68743ab543e45de072c8a6645788f0a64",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "CurrencyAllNull",
            "fresh-all-null",
            "insert-zero",
            101_u32,
            "h5-CurrencyAllNull-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
            "h5-CurrencyAllNull-insert-zero.accdb",
            "76c40c8e06d31a63880ee89a0a4a1d1324c51fbd843aed6d316e5aba76eb6bec",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "CurrencyAllNull",
            "fresh-all-null",
            "delete-all",
            101_u32,
            "h5-CurrencyAllNull-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
            "h5-CurrencyAllNull-delete-all.accdb",
            "814ecc80c10e29adcfa1a145ef837a75c52c72ee1f2a608c8b54acb3b215df84",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "Int64AllNull",
            "fresh-all-null",
            "insert-null",
            111_u32,
            "h6-Int64AllNull-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
            "h6-Int64AllNull-insert-null.accdb",
            "e615b7cee04b0ebcdc39a12cd87e5bd97f84772caccd1197cfaf0887df44dfd1",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "Int64AllNull",
            "fresh-all-null",
            "insert-zero",
            111_u32,
            "h6-Int64AllNull-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
            "h6-Int64AllNull-insert-zero.accdb",
            "00708ce0875a21490d16d46b98b4c1fbc994e0f612c009147c65eafc958ce0c1",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "Int64AllNull",
            "fresh-all-null",
            "delete-all",
            111_u32,
            "h6-Int64AllNull-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
            "h6-Int64AllNull-delete-all.accdb",
            "7ddbf86b818a8af4eb24b103907f262ace97f86accd5d0fb73b288a6d6c902de",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "CurrencyAllNull",
            "fresh-all-null",
            "insert-null",
            118_u32,
            "h6-CurrencyAllNull-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
            "h6-CurrencyAllNull-insert-null.accdb",
            "405c2da1bbe6443c67ed4dc4c3a9076458522400cf4c7eb8c26fa5c31865cabb",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "CurrencyAllNull",
            "fresh-all-null",
            "insert-zero",
            118_u32,
            "h6-CurrencyAllNull-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
            "h6-CurrencyAllNull-insert-zero.accdb",
            "5e59fb5666b9f51eae29233e209ec32228f2c6ab6ce7c1455671c7be3b71ff68",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "CurrencyAllNull",
            "fresh-all-null",
            "delete-all",
            118_u32,
            "h6-CurrencyAllNull-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
            "h6-CurrencyAllNull-delete-all.accdb",
            "3915d381b9cbb0d566f4b1e6beb2d01c9b0bd5574a1582e81e7d3b845e2e8e22",
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
    assert_eq!(cases.len(), 32);
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
    vec![Value::Int32(4), key, Value::Int32(68)]
}

fn deletes(case: &Case) -> Vec<RowDelete> {
    assert_ne!(case.family, "empty");
    (0..3)
        .map(|slot| {
            let start = case.data as usize * 4096 + 4077 - slot * 19;
            RowDelete::new(slot, case.before[start..start + 19].to_vec())
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
            body.extend_from_slice(&19_u32.to_le_bytes());
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
#[ignore = "requires44 derivative and48 fresh numeric source views"]
fn dense_empty_and_all_null_indexes_retain_literal_carriers_histories_and_wires() {
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
#[ignore = "requires44 derivative and48 fresh numeric source views"]
fn dense_nullable_families_recover_original_histories_from_pending_and_committed_pairs() {
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
        .filter(|case| case.recipe == "delete-all" && case.family == "fresh-all-null")
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
