//! Exact independent joint SOURCE artifacts; all loaded before target creation.
use std::{env, fs, path::PathBuf};

pub struct Case {
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub id: Option<i32>,
}

pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let originals = PathBuf::from(env::var_os("TABLECRAFT_PROPERTY_LVAL_ORIGINALS_DIR").unwrap());
    let expected =
        PathBuf::from(env::var_os("TABLECRAFT_OWNED_GROWTH_JOINT_EXPECTED_DIR").unwrap());
    assert!(originals.is_absolute() && expected.is_absolute());
    let mut cases = Vec::new();
    for (header, original_sha, null_sha, present_sha) in [
        (
            5,
            "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
            "a34e8084775960468e7c7dcb5605d6351e518ce9c463d545c847bfcc7117ccec",
            "dd706c5ff76ca1908374ded2982137cf0cb9f1fbc203d729033cf8587987bf2d",
        ),
        (
            6,
            "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
            "895be7c8b39403a3f6de38b224cab4c245aa732ff491f916b30c73333eb36c18",
            "445f68256d080d178d888f777c59aec22f7cf0984cb3d3b69c4ac69086cbf5c9",
        ),
    ] {
        let before = fs::read(originals.join(format!("h{header}.accdb"))).unwrap();
        verify(&before, original_sha);
        for (recipe, id, digest) in [
            ("id-null", None, null_sha),
            ("id-twenty-four", Some(24), present_sha),
        ] {
            let after =
                fs::read(expected.join(format!("h{header}-marker-growth-{recipe}.accdb"))).unwrap();
            verify(&after, digest);
            assert_eq!(before.len(), after.len());
            cases.push(Case {
                before: before.clone(),
                after,
                id,
            });
        }
    }
    assert_eq!(cases.len(), 4);
    cases
}
