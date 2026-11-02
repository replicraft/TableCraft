//! Four source-only direct-root deletion images, checked before target creation.
use std::{env, fs, path::PathBuf};

pub struct Case {
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub data: u32,
}

pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory = PathBuf::from(env::var_os("TABLECRAFT_NORMAL_DELETE_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut cases = Vec::new();
    for (header, length, before_sha, after_sha, data) in [
        (
            5,
            458752,
            "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
            "187714c3125969f8b47a50b1e83d993f3512fe21911bb0043e650b7dcd2754da",
            110,
        ),
        (
            6,
            528384,
            "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
            "de8da4c54fb46f384102900212376ca1c91768bdf8e6b2c153e4410e844ab03e",
            127,
        ),
    ] {
        let before =
            fs::read(directory.join(format!("h{header}-ordinary-delete-before.accdb"))).unwrap();
        let after =
            fs::read(directory.join(format!("h{header}-ordinary-delete-deleted.accdb"))).unwrap();
        assert_eq!(before.len(), length);
        assert_eq!(after.len(), length);
        verify(&before, before_sha);
        verify(&after, after_sha);
        cases.push(Case {
            before,
            after,
            data,
        });
    }
    assert_eq!(cases.len(), 2);
    cases
}
