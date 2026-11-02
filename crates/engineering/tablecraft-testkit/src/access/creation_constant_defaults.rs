//! Frozen source pins and original typed requests, never producer expectations.
use std::{fs, path::PathBuf};
use tablecraft_core::values::{Currency, Value};

pub const CASES: [&str; 4] = [
    "constants",
    "false-max-quoted",
    "nullable-defaults",
    "required-empty",
];

pub fn defaults(case: &str) -> Vec<Value> {
    match case {
        "constants" => vec![
            Value::Boolean(true),
            Value::Int64(i64::MIN),
            Value::Currency(Currency::from_scaled(-10001)),
            Value::Text("seed".into()),
        ],
        "false-max-quoted" => vec![
            Value::Boolean(false),
            Value::Int64(i64::MAX),
            Value::Currency(Currency::from_scaled(i64::MAX)),
            Value::Text("a\"bΔ".into()),
        ],
        "nullable-defaults" => vec![Value::Boolean(true), Value::Null, Value::Null, Value::Null],
        "required-empty" => vec![
            Value::Boolean(false),
            Value::Int64(-1),
            Value::Currency(Currency::from_scaled(0)),
            Value::Text(String::new()),
        ],
        _ => panic!("unknown frozen source case"),
    }
}

pub fn original_row() -> Vec<Value> {
    vec![
        Value::Int32(1),
        Value::Boolean(false),
        Value::Int64(7),
        Value::Currency(Currency::from_scaled(25000)),
        Value::Text("original".into()),
    ]
}

pub struct Sources {
    directory: PathBuf,
}

impl Sources {
    pub fn load(mut verify: impl FnMut(&[u8], &str)) -> Self {
        let directory = PathBuf::from(
            std::env::var_os("TABLECRAFT_CONSTANT_DEFAULT_SOURCE_DIR")
                .expect("explicit frozen source directory"),
        );
        assert!(directory.is_absolute());
        for (name, digest) in PINS {
            let bytes = fs::read(directory.join(name)).unwrap();
            assert_eq!(bytes.len(), 36864);
            verify(&bytes, digest);
        }
        Self { directory }
    }

    pub fn bytes(&self, case: &str, phase: &str) -> Vec<u8> {
        fs::read(self.directory.join(format!("h5-{case}-{phase}.accdb"))).unwrap()
    }
}

const PINS: &[(&str, &str)] = &[
    (
        "h5-constants-before.accdb",
        "1cdf0dc160eac689f86ad7846ccef49a86cd3f5deec889f7bb0f6f47476f03a4",
    ),
    (
        "h5-constants-omitted-constants.accdb",
        "48ae1b4dad2e90a787fbc9a14852efb1c87c224f6c0cfea820fc904d8cff5e5a",
    ),
    (
        "h5-constants-supplied-null.accdb",
        "50699478677be9f1a5e08ced480e778616eeba41bf981bcad027f819c91b4691",
    ),
    (
        "h5-constants-supplied-overrides.accdb",
        "24195d024d38a5cc01cef7ea168d449de04e2287914b12955e8ef1557c66d192",
    ),
    (
        "h5-false-max-quoted-before.accdb",
        "8aa9e1f59e3a02e5f2eac85b1b853dd4b17e7957f369b2a4698092afb7dc276a",
    ),
    (
        "h5-false-max-quoted-omitted-constants.accdb",
        "468e04b5a9dd9136ecf798547370ba88de84c3384c4cad7688484d4a0bb6ccd3",
    ),
    (
        "h5-false-max-quoted-supplied-null.accdb",
        "bc53bac345c6f888a800051e21ec9e6a226b11471ebd97448f1bd54cf63aeffd",
    ),
    (
        "h5-false-max-quoted-supplied-overrides.accdb",
        "51ec0d82747f3dbe3063b9166d5c87ca48dc7a22cf6e487b68d79b3448100505",
    ),
    (
        "h5-nullable-defaults-before.accdb",
        "9c7bbc1d2a6e6695da74c88f464009ac190cebeeae52f82fdbe3aecf3ebd8cac",
    ),
    (
        "h5-nullable-defaults-omitted-constants.accdb",
        "3c850825c47c9a73c3aaf6b683799e3047433626b8790fa16216d00527532126",
    ),
    (
        "h5-nullable-defaults-supplied-null.accdb",
        "cdacd561f38c9386163fcf28a5b541da5544174bdf7a97c82ae31d2265dab162",
    ),
    (
        "h5-nullable-defaults-supplied-overrides.accdb",
        "1a01842804720da70d9e8f281b24e06eafbca171f9771365a7cac610af867cf7",
    ),
    (
        "h5-required-empty-before.accdb",
        "e9c10e4e96ffa60bf0f07b0e350d80ceaf40c41ad197e67b1b93a0f5dd6de640",
    ),
    (
        "h5-required-empty-supplied-overrides.accdb",
        "6ef729c1fee02668e5c490560571631c820292b65408ae08f7b082b4f552564b",
    ),
    (
        "h6-constants-before.accdb",
        "01e30758d3b117661ac5f7e6f336e0f58ec3a22fbea13f936eff9b36b0bd203c",
    ),
    (
        "h6-constants-omitted-constants.accdb",
        "9b8da13469a7a5499cf76127917ed20559fbf83619829342d4c811d19d9ba955",
    ),
    (
        "h6-constants-supplied-null.accdb",
        "aa81c54b54ee83195abde7d427a7552faa24457942057d5cb268c62d3e05d14f",
    ),
    (
        "h6-constants-supplied-overrides.accdb",
        "95b2958e26bdf76a18809de77f5f31fae1ba60027d65540bba1181850192cc01",
    ),
    (
        "h6-false-max-quoted-before.accdb",
        "388aa9ae473ff776bd148b3e784bad49ece9980c1f597e2dcfe92de6aa654aca",
    ),
    (
        "h6-false-max-quoted-omitted-constants.accdb",
        "cc2b483ce3131adab84ba866e71792d33d2ac2104a30749a24e62cc6ed8ea561",
    ),
    (
        "h6-false-max-quoted-supplied-null.accdb",
        "4e8f9d642f7ccc4c07ae1623a9970eb0f8e82804c8f52857e929ad817aa5002f",
    ),
    (
        "h6-false-max-quoted-supplied-overrides.accdb",
        "9913e3ff65b524ce01532d6d0b52982f5e27af6632d4fd73c6330509584b13ce",
    ),
    (
        "h6-nullable-defaults-before.accdb",
        "e5cedfaf08199e478058677301cb0356ef479bfbb9e81ef43510e19e57db4ecb",
    ),
    (
        "h6-nullable-defaults-omitted-constants.accdb",
        "3f43448c5e896e9ea86f5e6379d3ea77b4b304c7ccbcb18757e951864c1c3919",
    ),
    (
        "h6-nullable-defaults-supplied-null.accdb",
        "3747f50d0a383eb27755d13da1e9e1e5ac2e109e6c01fa7d1416a2150b3effef",
    ),
    (
        "h6-nullable-defaults-supplied-overrides.accdb",
        "c6b41d408e1b3d2d1c434cab510186857e0f5aa35bf219074172c80c07376414",
    ),
    (
        "h6-required-empty-before.accdb",
        "d735aaeab67dca779cd55facd639ab7c25ce5537327cdd294e1efd80bc201b22",
    ),
    (
        "h6-required-empty-supplied-overrides.accdb",
        "b37b41e64ad3861873706e837fbf2c555694568eff7d2a482114023d56534de5",
    ),
];
