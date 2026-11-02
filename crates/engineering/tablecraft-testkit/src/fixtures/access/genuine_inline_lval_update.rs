//! Independent original/request/literal images for real inline fitting consumers.
use std::{fs, path::PathBuf};
use tablecraft_core::values::Value;

const PINS: &[(u8, &str, &str, usize, &str)] = &[
    (
        5,
        "original",
        "h5-inline-update-original.accdb",
        380928,
        "7a08edca1f32c58ce2451fc6056f257bf7d59fa2483a92b628682d06f5bb9861",
    ),
    (
        5,
        "noop",
        "h5-inline-update-noop.accdb",
        380928,
        "7a08edca1f32c58ce2451fc6056f257bf7d59fa2483a92b628682d06f5bb9861",
    ),
    (
        5,
        "memo-new",
        "h5-inline-update-memo-new.accdb",
        380928,
        "3c7d00fee18f974fdfd9fe1caf0b3155df9cfc81b3f7ea2bc2607706d5d7f8c5",
    ),
    (
        5,
        "raw-unicode",
        "h5-inline-update-raw-unicode.accdb",
        380928,
        "9a4e901a3abde63ddf2850cd59f5c29145f8227a81e9f06f2617bc68a7dbed9d",
    ),
    (
        5,
        "memo-empty",
        "h5-inline-update-memo-empty.accdb",
        380928,
        "6e85b778edcc45fd4bd272027ddaffa33670eeff5bff099ab842ceaad23b9713",
    ),
    (
        5,
        "memo-clear",
        "h5-inline-update-memo-clear.accdb",
        380928,
        "7dbc3ea69cf00c557f9499b951b0feb93f872eedbacebc923a64113b46ca7df8",
    ),
    (
        5,
        "blob-empty",
        "h5-inline-update-blob-empty.accdb",
        380928,
        "1e83c986b505cf007467573eccc33a2980ead3a00ff49dd799fe5d309e09e50c",
    ),
    (
        5,
        "blob-clear",
        "h5-inline-update-blob-clear.accdb",
        380928,
        "b600068df25218f44f2d1664a7f5c154512c9a01e01aee65360d092273237da2",
    ),
    (
        5,
        "both-empty",
        "h5-inline-update-both-empty.accdb",
        380928,
        "c92445ef6dfcab411be5d4733cd8b19797816fce2a2b2aaa45542aae015685ec",
    ),
    (
        5,
        "both-null",
        "h5-inline-update-both-null.accdb",
        380928,
        "36ffa06afeed513904832040a0aad4221605e886dadc2b0ae305552458219728",
    ),
    (
        5,
        "shared-exact",
        "h5-inline-update-shared-exact.accdb",
        380928,
        "2867a6395e8f19cb134cf9c788b45a9dadf6fab1e5d0dbd1b6a228a7e04fe071",
    ),
    (
        5,
        "blob-exact",
        "h5-inline-update-blob-exact.accdb",
        380928,
        "7baed1e088c48fa4b6edc650b8789057e58ad70ee7f058a5611a83fefecb281d",
    ),
    (
        5,
        "fixed-joint",
        "h5-inline-update-fixed-joint.accdb",
        380928,
        "4fc04ed6f23b52bb06ab93ff1179517d17760aa67cb5541c8b9661a0062ceb42",
    ),
    (
        5,
        "empty-group",
        "h5-inline-update-empty-group.accdb",
        380928,
        "7a08edca1f32c58ce2451fc6056f257bf7d59fa2483a92b628682d06f5bb9861",
    ),
    (
        6,
        "original",
        "h6-inline-update-original.accdb",
        450560,
        "3f1bbfc5afcc355ea40a9906ab71a25456fb34e572d15090467211874dff3325",
    ),
    (
        6,
        "noop",
        "h6-inline-update-noop.accdb",
        450560,
        "3f1bbfc5afcc355ea40a9906ab71a25456fb34e572d15090467211874dff3325",
    ),
    (
        6,
        "memo-new",
        "h6-inline-update-memo-new.accdb",
        450560,
        "186b2aeaaa4b6b2b39b97e99103141ab62167b020e33a5766dc52b9668604e78",
    ),
    (
        6,
        "raw-unicode",
        "h6-inline-update-raw-unicode.accdb",
        450560,
        "4fccf903571d1d7f6a2a276bd332c99392234d03dc0db3ddaf6ffa05ea376674",
    ),
    (
        6,
        "memo-empty",
        "h6-inline-update-memo-empty.accdb",
        450560,
        "2bcc01ff40cece2b6a2db0b58067f2abfd1d13a2030a321f9fee2298bfd8e68d",
    ),
    (
        6,
        "memo-clear",
        "h6-inline-update-memo-clear.accdb",
        450560,
        "41ba9181626b2f6bf8a353d0d73605e20916e12dbb967fe9383bcca868db7487",
    ),
    (
        6,
        "blob-empty",
        "h6-inline-update-blob-empty.accdb",
        450560,
        "8ab484da97cd606695f915403aecb8bf687c26518fff8684e5303308fbb40e4f",
    ),
    (
        6,
        "blob-clear",
        "h6-inline-update-blob-clear.accdb",
        450560,
        "9fce56f556285e3f1b73a9f8e09312083b19b3763231cb5756884b7a9d93f75a",
    ),
    (
        6,
        "both-empty",
        "h6-inline-update-both-empty.accdb",
        450560,
        "f55f6d41c175fd3588b3009d2f36a464f3052091884b869cd190720e60120996",
    ),
    (
        6,
        "both-null",
        "h6-inline-update-both-null.accdb",
        450560,
        "534461ccf69122a1f470013de1c41a63042d3550f022ec8368ee219ff6b5e038",
    ),
    (
        6,
        "shared-exact",
        "h6-inline-update-shared-exact.accdb",
        450560,
        "2ec90c96ff8f4d9fb42f7a2fc06384fcdd9f49f75921884d5ca74099dc73228d",
    ),
    (
        6,
        "blob-exact",
        "h6-inline-update-blob-exact.accdb",
        450560,
        "c77c829a4b5cc4b7c512df3dc3794f811829236a1e0d59a2415648166f093a6c",
    ),
    (
        6,
        "fixed-joint",
        "h6-inline-update-fixed-joint.accdb",
        450560,
        "f83fe5e0d366347767e76a1eecdb8af427710be314e228ef5927656baa244885",
    ),
    (
        6,
        "empty-group",
        "h6-inline-update-empty-group.accdb",
        450560,
        "3f1bbfc5afcc355ea40a9906ab71a25456fb34e572d15090467211874dff3325",
    ),
];

struct Recipe {
    name: &'static str,
    memo: Option<&'static str>,
    blob: Option<&'static [u8]>,
    id: i32,
    empty: bool,
}

const RECIPES: &[Recipe] = &[
    Recipe {
        name: "noop",
        memo: Some("old"),
        blob: Some(&[199, 0, 255]),
        id: 1,
        empty: false,
    },
    Recipe {
        name: "memo-new",
        memo: Some("new"),
        blob: Some(&[199, 0, 255]),
        id: 1,
        empty: false,
    },
    Recipe {
        name: "raw-unicode",
        memo: Some("βγ"),
        blob: Some(&[199, 0, 255]),
        id: 1,
        empty: false,
    },
    Recipe {
        name: "memo-empty",
        memo: Some(""),
        blob: Some(&[199, 0, 255]),
        id: 1,
        empty: false,
    },
    Recipe {
        name: "memo-clear",
        memo: None,
        blob: Some(&[199, 0, 255]),
        id: 1,
        empty: false,
    },
    Recipe {
        name: "blob-empty",
        memo: Some("old"),
        blob: Some(&[]),
        id: 1,
        empty: false,
    },
    Recipe {
        name: "blob-clear",
        memo: Some("old"),
        blob: None,
        id: 1,
        empty: false,
    },
    Recipe {
        name: "both-empty",
        memo: Some(""),
        blob: Some(&[]),
        id: 1,
        empty: false,
    },
    Recipe {
        name: "both-null",
        memo: None,
        blob: None,
        id: 1,
        empty: false,
    },
    Recipe {
        name: "shared-exact",
        memo: Some("abcdef"),
        blob: Some(&[]),
        id: 1,
        empty: false,
    },
    Recipe {
        name: "blob-exact",
        memo: None,
        blob: Some(&[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
        ]),
        id: 1,
        empty: false,
    },
    Recipe {
        name: "fixed-joint",
        memo: Some("new"),
        blob: Some(&[0, 255, 0]),
        id: 7,
        empty: false,
    },
    Recipe {
        name: "empty-group",
        memo: Some("old"),
        blob: Some(&[199, 0, 255]),
        id: 1,
        empty: true,
    },
];

pub struct Update {
    pub expected: Value,
    pub replacement: Value,
}

pub struct Group {
    pub column: &'static str,
    pub fixed: bool,
    pub updates: Vec<Update>,
}

pub struct Case {
    pub header: u8,
    pub recipe: &'static str,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub definition: u32,
    pub data: u32,
    pub expected: Vec<Value>,
    pub groups: Vec<Group>,
    pub changed: usize,
}

pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_INLINE_LVAL_UPDATE_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let manifest = fs::read(directory.join("manifest.json")).unwrap();
    verify(
        &manifest,
        "eacf49ab1528ae1c46b81c191a0c40f3b94468771ae4aeff7a8170a2411d2f80",
    );
    // Eagerly pin the complete independent corpus before returning any case.
    let mut images = Vec::new();
    for &(header, recipe, filename, bytes, sha) in PINS {
        let raw = fs::read(directory.join(filename)).unwrap();
        assert_eq!(raw.len(), bytes);
        verify(&raw, sha);
        images.push((header, recipe, raw));
    }
    let mut result = Vec::new();
    for header in [5, 6] {
        let before = &images
            .iter()
            .find(|(h, name, _)| *h == header && *name == "original")
            .unwrap()
            .2;
        for recipe in RECIPES {
            let memo = recipe
                .memo
                .map(|text| Value::Text(text.into()))
                .unwrap_or(Value::Null);
            let blob = recipe
                .blob
                .map(|bytes| Value::Binary(bytes.into()))
                .unwrap_or(Value::Null);
            let mut groups = vec![Group {
                column: "MemoValue",
                fixed: false,
                updates: if recipe.empty {
                    Vec::new()
                } else {
                    vec![Update {
                        expected: Value::Text("old".into()),
                        replacement: memo.clone(),
                    }]
                },
            }];
            if !recipe.empty {
                groups.push(Group {
                    column: "BlobValue",
                    fixed: false,
                    updates: vec![Update {
                        expected: Value::Binary(vec![199, 0, 255]),
                        replacement: blob.clone(),
                    }],
                });
                if recipe.id != 1 {
                    groups.push(Group {
                        column: "Id",
                        fixed: true,
                        updates: vec![Update {
                            expected: Value::Int32(1),
                            replacement: Value::Int32(recipe.id),
                        }],
                    });
                }
            }
            let after = &images
                .iter()
                .find(|(h, name, _)| *h == header && *name == recipe.name)
                .unwrap()
                .2;
            result.push(Case {
                header,
                recipe: recipe.name,
                before: before.clone(),
                after: after.clone(),
                definition: if header == 5 { 89 } else { 106 },
                data: if header == 5 { 91 } else { 108 },
                expected: vec![Value::Int32(recipe.id), memo, blob],
                groups,
                changed: usize::from(!matches!(recipe.name, "noop" | "empty-group")),
            });
        }
    }
    assert_eq!(result.len(), 26);
    result
}
