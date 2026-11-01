//! Six exact source resources, all pinned before any target-owning consumer runs.
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

pub(super) struct Case {
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub no_capacity: Vec<u8>,
    pub pages: [u32; 3],
}

pub(super) fn cases() -> Vec<Case> {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_MULTIROW_OWNED_EXPECTED_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut cases = Vec::new();
    for (header, length, pins, pages) in [
        (
            5_u8,
            466944,
            [
                "be0323af76ec000fd6eaace6b4595c4568dfed31ea83d83f85b067777f712f57",
                "ce6411064a66d2ca1c56138f25123ba69b694db3a50f72ce5cbe64744a3e7250",
                "2515f4fe4977005335f5aa68a6375ade70b75ffd0dd9f552fa4e62c249a27888",
            ],
            [110_u32, 112, 113],
        ),
        (
            6_u8,
            536576,
            [
                "41f5d0d9fb4c1a997fcd8072855fd72f1183923a76f1615229825bfd76e5e257",
                "eddbff288ae4c27b2468d5596afb938d49ebe5fac9e2c4cf9ed7d7b64bcd33eb",
                "cbfaca8d36102e477d3aa7a7b86830169de70b5a64ff6d24b10f2855e81a9bde",
            ],
            [127_u32, 129, 130],
        ),
    ] {
        let mut images = Vec::new();
        for (suffix, pin) in ["before", "growth", "no-capacity"].into_iter().zip(pins) {
            let raw = fs::read(directory.join(format!("h{header}-multirow-owned-{suffix}.accdb")))
                .unwrap();
            assert_eq!(raw.len(), length);
            let actual = Sha256::digest(&raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            assert_eq!(actual, pin);
            images.push(raw);
        }
        let no_capacity = images.pop().unwrap();
        let after = images.pop().unwrap();
        let before = images.pop().unwrap();
        cases.push(Case {
            before,
            after,
            no_capacity,
            pages,
        });
    }
    assert_eq!(cases.len(), 2);
    cases
}
