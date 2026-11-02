//! Source-authored escaped INSERT images and separate reader-alias controls.
use std::{env, fs, path::PathBuf};
use tablecraft_core::values::Value;

/// One header's immutable original and both independently authored Text wires.
pub struct Case {
    /// Physical format header word, 5 or 6.
    pub header: u8,
    /// Selected table definition page.
    pub definition: u32,
    /// Existing owned DATA page retaining the old row.
    pub data: u32,
    /// Complete unchanged genuine original.
    pub before: Vec<u8>,
    /// Correct escaped representation of the original requested FEFF string.
    pub after: Vec<u8>,
    /// Readable unescaped wire observing `new`, not the requested FEFF+new.
    pub alias: Vec<u8>,
}

/// Original complete supplied values, without any native serialization.
pub fn values() -> Vec<Value> {
    vec![
        Value::Int32(24),
        Value::Text("\u{feff}new".into()),
        Value::Null,
    ]
}

/// Verify all six image pins before returning any case to a native consumer.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory = PathBuf::from(env::var_os("TABLECRAFT_NORMAL_INSERT_FEFF_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut cases = Vec::new();
    for (header, definition, data, length, before_pin, after_pin, alias_pin) in [
        (
            5,
            107,
            110,
            458752,
            "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
            "a6dc745e868883362eec9aea04d84bc08e80265be2bebe67d43456808a16eb06",
            "2d37ba1b723dea47464120cfc07e10b4153d5f416353fc8924df44d4bf457ff9",
        ),
        (
            6,
            125,
            127,
            528384,
            "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
            "87a3062e2b8473407e8bd6be8997a0fa3c80094fd6adf80da784b8422f638fad",
            "60a1b1c5ac1934d65f09cbf2c557ced29702f1e5ca9d7d748d3c979b546bd3bd",
        ),
    ] {
        let prefix = format!("h{header}-insert-feff");
        let before = fs::read(directory.join(format!("{prefix}-before.accdb"))).unwrap();
        let after = fs::read(directory.join(format!("{prefix}-escaped.accdb"))).unwrap();
        let alias = fs::read(directory.join(format!("{prefix}-unescaped-alias.accdb"))).unwrap();
        for (raw, expected) in [
            (&before, before_pin),
            (&after, after_pin),
            (&alias, alias_pin),
        ] {
            assert_eq!(raw.len(), length);
            verify(raw, expected);
        }
        cases.push(Case {
            header,
            definition,
            data,
            before,
            after,
            alias,
        });
    }
    assert_eq!(cases.len(), 2);
    cases
}
