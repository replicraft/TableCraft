//! The executable retains argument names while parsed values move into bindings.
use super::parse;
use crate::commands::query::Error;
use tablecraft_core::values::Value;

#[test]
fn bindings_borrow_original_names_and_retain_exact_owned_values() {
    let raw = [
        "[P=Q]=text:a=b:c".to_owned(),
        "Nothing=null:".to_owned(),
        "Count=long:-17".to_owned(),
    ];
    let Ok(bindings) = parse(&raw) else {
        panic!("valid bounded bindings must parse");
    };
    assert_eq!(bindings[0].0, "P=Q");
    assert_eq!(bindings[0].0.as_ptr(), raw[0][1..4].as_ptr());
    assert_eq!(bindings[0].1, Value::Text("a=b:c".into()));
    assert_eq!(bindings[1].0.as_ptr(), raw[1].as_ptr());
    assert_eq!(bindings[1].1, Value::Null);
    assert_eq!(bindings[2].1, Value::Int32(-17));
}

#[test]
fn borrowed_names_keep_duplicate_and_invalid_name_refusals() {
    for raw in [
        vec!["P=long:1".to_owned(), "p=text:other".to_owned()],
        vec!["[P]=long:1".to_owned(), "p=null:".to_owned()],
        vec!["=long:1".to_owned()],
        vec!["[P[Q]=long:1".to_owned()],
        vec!["P\nQ=long:1".to_owned()],
    ] {
        assert!(matches!(parse(&raw), Err(Error::Parameter)));
    }
}
