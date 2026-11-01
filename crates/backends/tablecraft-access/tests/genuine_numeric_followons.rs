#![cfg(all(
    feature = "offline-write-session",
    any(target_os = "macos", target_os = "linux")
))]

use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tablecraft_access::editing::offline::{OfflineWriteSession, SessionState};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Value},
};
use tablecraft_db::{DeletableTable, EditableTable, InsertableTable};
use tablecraft_io::{ExclusiveFile, IoError, journal::RecoveryOutcome};
use tablecraft_model::CatalogScope;
use tablecraft_sql::{ParseLimits, parse_query};

const SCOPE: CatalogScope = CatalogScope::new(201);

struct Case {
    table: &'static str,
    family: &'static str,
    recipe: &'static str,
    before: Vec<u8>,
    after: Vec<u8>,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn cases() -> Vec<Case> {
    let mut result = Vec::new();
    for (env, table, family, recipe, before, oldhash, after, newhash) in [
        (
            "TABLECRAFT_INTEGRAL_FOLLOWON_SOURCE_DIR",
            "Int64Keys",
            "empty",
            "insert-null",
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
            "h5-CurrencyAllNull-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
            "h5-CurrencyAllNull-delete-all.accdb",
            "814ecc80c10e29adcfa1a145ef837a75c52c72ee1f2a608c8b54acb3b215df84",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "Int64WidePrimary",
            "wide-primary",
            "insert-null",
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
            "h5-CurrencyWidePrimary-before.accdb",
            "dff3f189f583a847562405415f7bb16a28ca461c5e0db0a61d499cab8ca2487e",
            "h5-CurrencyWidePrimary-delete-all.accdb",
            "830586432a31c26c3d687d7287750af5a104c5c9d8a99fc415805f7b250c0293",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "Int64AllNull",
            "fresh-all-null",
            "insert-null",
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
            "h6-CurrencyAllNull-before.accdb",
            "6a928bbb25cb98bb1d1da0965b00012fac3d610ea5d294d266aa791323cc07a7",
            "h6-CurrencyAllNull-delete-all.accdb",
            "3915d381b9cbb0d566f4b1e6beb2d01c9b0bd5574a1582e81e7d3b845e2e8e22",
        ),
        (
            "TABLECRAFT_INTEGRAL_EXTRA_FOLLOWON_DIR",
            "Int64WidePrimary",
            "wide-primary",
            "insert-null",
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
        result.push(Case {
            table,
            family,
            recipe,
            before,
            after,
        });
    }
    assert_eq!(result.len(), 44);
    result
}

fn scalar(case: &Case, units: i64) -> Value {
    if case.table.starts_with("Currency") {
        Value::Currency(Currency::from_scaled(units))
    } else {
        Value::Int64(units)
    }
}

fn request(case: &Case) -> Vec<Value> {
    vec![
        if case.family == "wide-primary" {
            scalar(case, 1)
        } else {
            Value::Int32(4)
        },
        if case.recipe == "insert-null" {
            Value::Null
        } else {
            scalar(case, 0)
        },
        Value::Int32(68),
    ]
}

fn expected_rows(case: &Case) -> Vec<Vec<Value>> {
    if case.recipe == "delete-all" {
        return Vec::new();
    }
    let mut rows = Vec::new();
    if case.family != "empty" {
        for (slot, marker) in [17, 34, 51].into_iter().enumerate() {
            let id = if case.family == "wide-primary" {
                scalar(case, [i64::MIN, 0, i64::MAX][slot])
            } else {
                Value::Int32(slot as i32 + 1)
            };
            let key = if case.family == "wide-primary" && slot != 1 {
                scalar(case, if slot == 0 { i64::MIN } else { i64::MAX })
            } else {
                Value::Null
            };
            rows.push(vec![id, key, Value::Int32(marker)]);
        }
    }
    rows.push(request(case));
    rows
}

fn unchanged(path: &Path, bundle: &Path, before: &[u8]) {
    assert_eq!(fs::read(path).unwrap(), before);
    assert!(!bundle.exists());
    assert!(matches!(ExclusiveFile::open(path), Err(IoError::LockBusy)));
}

#[test]
#[ignore = "requires source104 native contexts before matching producers"]
fn numeric_followon_typed_and_honest_sql_actions_preserve_complete_sources_and_receipts() {
    let mut saves = [0; 2];
    for case in cases() {
        for typed in [true, false] {
            let insert = case.recipe.starts_with("insert-");
            if !typed && insert && (case.family == "wide-primary" || case.recipe != "insert-null") {
                continue;
            }
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("target.accdb");
            let bundle = dir.path().join("journal");
            fs::write(&path, &case.before).unwrap();
            let session =
                OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
            let changed = if insert { 1 } else { 3 };
            let (session, receipt) = if insert {
                let staged = if typed {
                    session
                        .insert_table(case.table, &mut budget(), &|| false)
                        .unwrap()
                        .plan_insert(request(&case), &mut budget(), &|| false)
                        .unwrap()
                } else {
                    let sql = format!(
                        "INSERT INTO {} ([Marker],[KeyValue],[Id]) VALUES (68,NULL,4)",
                        case.table
                    );
                    let query = parse_query(&sql, ParseLimits::default()).unwrap();
                    session
                        .plan_insert(query.query(), &mut budget(), &|| false)
                        .unwrap()
                };
                assert_eq!(staged.inserted_rows(), 1);
                unchanged(&path, &bundle, &case.before);
                staged.apply(&bundle, &mut budget(), &|| false).unwrap()
            } else {
                let staged = if typed {
                    let editor = session
                        .edit_table(case.table, &mut budget(), &|| false)
                        .unwrap();
                    let snapshot = EditableTable::snapshot(&editor);
                    assert_eq!(snapshot.rows().len(), 3);
                    let handles = (0..3)
                        .map(|ordinal| snapshot.row_handle(ordinal).unwrap())
                        .collect::<Vec<_>>();
                    DeletableTable::plan_deletes(editor, &handles, &mut budget(), &|| false)
                        .unwrap()
                } else {
                    let query = parse_query(
                        &format!("DELETE FROM {}", case.table),
                        ParseLimits::default(),
                    )
                    .unwrap();
                    session
                        .plan_delete(query.query(), &mut budget(), None, &|| false)
                        .unwrap()
                };
                assert_eq!((staged.matched_rows(), staged.deleted_rows()), (3, 3));
                unchanged(&path, &bundle, &case.before);
                staged.apply(&bundle, &mut budget(), &|| false).unwrap()
            };
            assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
            assert_eq!(receipt.changed_rows(), Some(changed));
            assert_eq!(receipt.matched_rows(), if insert { None } else { Some(3) });
            assert_eq!(session.last_completed(), Some(receipt));
            assert_eq!(session.state(), SessionState::Ready);
            assert_eq!(fs::read(&path).unwrap(), case.after);
            assert!(!bundle.exists());
            assert!(matches!(ExclusiveFile::open(&path), Err(IoError::LockBusy)));
            let editor = session
                .edit_table(case.table, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                EditableTable::snapshot(&editor).rows(),
                expected_rows(&case)
            );
            editor.into_session().release().unwrap();
            ExclusiveFile::open(&path).unwrap().close().unwrap();
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
            saves[usize::from(!typed)] += 1;
        }
    }
    assert_eq!(saves, [44, 24]);
}

#[test]
#[ignore = "requires source104 native contexts before matching producers"]
fn qualified_empty_numeric_tables_keep_zero_delete_receipts_and_complete_bytes() {
    let mut saves = 0;
    for case in cases()
        .into_iter()
        .filter(|case| case.family == "empty" && case.recipe == "insert-null")
    {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target.accdb");
        let bundle = dir.path().join("journal");
        fs::write(&path, &case.before).unwrap();
        let session = OfflineWriteSession::open(&path, SCOPE, &mut budget(), &|| false).unwrap();
        let editor = session
            .edit_table(case.table, &mut budget(), &|| false)
            .unwrap();
        assert!(EditableTable::snapshot(&editor).rows().is_empty());
        let staged = DeletableTable::plan_deletes(editor, &[], &mut budget(), &|| false).unwrap();
        assert_eq!((staged.matched_rows(), staged.deleted_rows()), (0, 0));
        unchanged(&path, &bundle, &case.before);
        let (session, receipt) = staged.apply(&bundle, &mut budget(), &|| false).unwrap();
        assert_eq!(receipt.matched_rows(), Some(0));
        assert_eq!(receipt.changed_rows(), Some(0));
        assert_eq!(receipt.outcome(), Some(RecoveryOutcome::Committed));
        assert_eq!(session.last_completed(), Some(receipt));
        assert_eq!(session.state(), SessionState::Ready);
        unchanged(&path, &bundle, &case.before);
        session.release().unwrap();
        ExclusiveFile::open(&path).unwrap().close().unwrap();
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        saves += 1;
    }
    assert_eq!(saves, 4);
}
