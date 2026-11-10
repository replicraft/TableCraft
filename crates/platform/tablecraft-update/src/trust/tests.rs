use super::{key_bytes, parse_key, release_key};

#[test]
fn maintained_production_key_parses_and_differs_from_public_test_key_material() {
    let production = include_str!("../../../../../packaging/update-key.pub");
    let test = include_str!("../download/tests/TEST-ONLY.pub");
    assert_ne!(
        key_bytes(production).unwrap()[10..],
        key_bytes(test).unwrap()[10..]
    );
    release_key().expect("the compiled release key must be provisioned and distinct");
}

#[test]
fn malformed_key_headers_and_extra_lines_refuse() {
    let test = include_str!("../download/tests/TEST-ONLY.pub");
    assert!(parse_key(test).is_ok());
    assert!(parse_key(&test.replace("untrusted comment:", "unexpected:")).is_err());
    assert!(parse_key(&format!("{test}extra\n")).is_err());
}
