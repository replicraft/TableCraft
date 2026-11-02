//! Pins captured from the first SOURCE export, before indexed producer calls.
use sha2::{Digest, Sha256};
use tablecraft_testkit::fixtures::access::canonical_indexed_before_change::{Recipe, source};

const PINS: &[(u8, Recipe, &str, Option<&str>)] = &[
    (
        5,
        Recipe::Ascending,
        "dbc9cc582b09c100b8de6d08bfe3b976adb3cdba22870dd15084a116d684170c",
        Some("aa8056782d70fe9be66e36a601b946aeefbd1866998aacfb51e6a85ff758bd97"),
    ),
    (
        5,
        Recipe::Descending,
        "d88df7ef5c2a82e7ad910d76c33c541ad412097a38fd31cd01a62416a19c585f",
        Some("36aed0f4f1aadc12f8ba9523dcb985de96f04834833c86dd9b36d1725158d0c2"),
    ),
    (
        5,
        Recipe::Composite,
        "c474f45071d48cd6779c5f168e810f0cbba7c258107bdc227da1c3807bfb7b40",
        Some("b84de884b2474ea54409ede648ba3dd778037ae1218917d4d7dce4b6af0f4120"),
    ),
    (
        5,
        Recipe::Alias,
        "30c32cae812adc1d285c2f0c9fc81c98cb3973f11112421a8ee8f4859f64811d",
        Some("9beca7e1b4bb0d6f1a4606d9e76f29d8645321c2d673be7aca50fea215adbd19"),
    ),
    (
        5,
        Recipe::UniqueSwap,
        "9b8a46c4d0983d8f6deed5689cf6aa305ab81bf4eebcd7f793ed178c3f32703c",
        Some("4e948e170b20306d09ccb009c0e81faa82ff7c72f85239dea2c8bddcc9a7fad0"),
    ),
    (
        5,
        Recipe::MacroOnlyId,
        "ea0af73e9fc7ba1a6b374c3443c1b1a5d58fd54a3411af23880b4ca9a67efc6f",
        Some("57660b6255fe595c09157720f9cd8398696fd88293286b158fd830260a69e8c8"),
    ),
    (
        5,
        Recipe::RestoreOriginal,
        "8a6e8b042cdf1277876506558fb7fbef3d563e1468fb9074cd3b34671ee1d24b",
        Some("8a6e8b042cdf1277876506558fb7fbef3d563e1468fb9074cd3b34671ee1d24b"),
    ),
    (
        5,
        Recipe::Empty,
        "c965032cf68ea07c1bbf0e6252e7c77458519497d11eb330f432e341c67fc007",
        Some("c965032cf68ea07c1bbf0e6252e7c77458519497d11eb330f432e341c67fc007"),
    ),
    (
        5,
        Recipe::UniqueConflict,
        "9b8a46c4d0983d8f6deed5689cf6aa305ab81bf4eebcd7f793ed178c3f32703c",
        None,
    ),
    (
        6,
        Recipe::Ascending,
        "c66b6bd0572080cffa505c87e4279b3a53328ed148aed67383e81e8d50b1e6d5",
        Some("4631fc2163cc7d9b4c921b3f0f8d06e60feec88b9c2cd35c7fdec6386d680c53"),
    ),
    (
        6,
        Recipe::Descending,
        "da547f24145992e48f91dcf845d49a9c57b2ecd171e0ed5e07c92f4f9f9d2353",
        Some("1ddc8467ba2bf37c70450b13cf30ff9b0ec929235e2b29649d9c6b48184d07d9"),
    ),
    (
        6,
        Recipe::Composite,
        "0c5f10d5f037dfaa602d783fffe0a1bab0bf41b0a2cf6f995bffe1dfcfe0734e",
        Some("aafe1ba7f4aee04faf099a0610bdcfd600508a3f15a7ef37c9475537d1d9bc59"),
    ),
    (
        6,
        Recipe::Alias,
        "a9f4d21d32a2d8e1bcba6674f2bdd467906ab01eee621362215ee275f73dc915",
        Some("94ee6123cd5c7efdb1778b4db1f61e6bcfcb81921c3e4cfca2b3e9f5695c7d25"),
    ),
    (
        6,
        Recipe::UniqueSwap,
        "194f7d9b2a5441e41277ff55e339dcbfa97a7f86f79006bb43553df8133f6b50",
        Some("40488192fa91441de7758ff7b7419eb886c5cd9c90bb1e84de91e044b90eb91f"),
    ),
    (
        6,
        Recipe::MacroOnlyId,
        "b543db3dc662840f049245f64d2fdcdb99f5c9e8a03bb6762a165eb4daad4643",
        Some("d5d4990fa3a05e14058529c1df918207622f1b73b0a2275d462a94a3cfc87d23"),
    ),
    (
        6,
        Recipe::RestoreOriginal,
        "33007f1a25d6f42e95251da674f666a9c7eeb5a72cbbb8bb509d6fe44ad8a7fd",
        Some("33007f1a25d6f42e95251da674f666a9c7eeb5a72cbbb8bb509d6fe44ad8a7fd"),
    ),
    (
        6,
        Recipe::Empty,
        "e32986c7b38ecd835e258c9cb600f4bc634675d65659e7064b5754ece4f75397",
        Some("e32986c7b38ecd835e258c9cb600f4bc634675d65659e7064b5754ece4f75397"),
    ),
    (
        6,
        Recipe::UniqueConflict,
        "194f7d9b2a5441e41277ff55e339dcbfa97a7f86f79006bb43553df8133f6b50",
        None,
    ),
];

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn all_indexed_originals_outcomes_and_original_requests_retain_first_export_pins() {
    let mut requests = Sha256::new();
    let mut views = 0;
    for &(header, recipe, original, expected) in PINS {
        let case = source(header, recipe);
        assert_eq!(digest(&case.before), original);
        views += 1;
        match (case.expected.as_deref(), expected) {
            (Some(bytes), Some(pin)) => {
                assert_eq!(digest(bytes), pin);
                views += 1;
            }
            (None, None) => {}
            _ => panic!("SOURCE role changed"),
        }

        let name = recipe.id().as_bytes();
        requests.update([header]);
        requests.update((name.len() as u64).to_le_bytes());
        requests.update(name);
        requests.update((case.updates.len() as u64).to_le_bytes());
        for update in &case.updates {
            requests.update((update.ordinal as u64).to_le_bytes());
            requests.update(update.expected.to_le_bytes());
            requests.update(update.replacement.to_le_bytes());
        }
    }
    assert_eq!(views, 34);
    let actual = requests
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        actual,
        "a7e43b5ff8cc1ef6f350a25ef6400379da2c738bc389ecbf8f91298641c4b8c4"
    );
}
