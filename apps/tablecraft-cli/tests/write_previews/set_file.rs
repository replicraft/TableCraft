//! Fixed-file transport parity uses the already qualified actual source owner.
use super::{fs, invoke, model, rows, source, success};
use tablecraft_core::values::Value;

#[test]
fn file_fixed_dispatch_matches_existing_inline_request_from_the_same_source() {
    let directory = tempfile::tempdir().unwrap();
    let before = model::fixed(false);
    let input = source(directory.path(), &before);
    let file_output = directory.path().join("file.accdb");
    let inline_output = directory.path().join("inline.accdb");
    let requests = directory.path().join("requests.tsv");
    fs::write(&requests, "Items\tScore\t1\tint32\t20\t25\n").unwrap();
    success(invoke(&[
        "write",
        "preview-update",
        input.to_str().unwrap(),
        file_output.to_str().unwrap(),
        "--set-file",
        requests.to_str().unwrap(),
        "--index-growth",
    ]));
    success(invoke(&[
        "write",
        "preview-update",
        input.to_str().unwrap(),
        inline_output.to_str().unwrap(),
        "--set",
        "Items",
        "Score",
        "1",
        "int32",
        "20",
        "25",
        "--index-growth",
    ]));
    let file = fs::read(file_output).unwrap();
    let inline = fs::read(inline_output).unwrap();
    assert!(file == inline, "file/argv fixed dispatch parity");
    assert_eq!(rows(&file)[1][1], Value::Int32(25));
    assert!(fs::read(input).unwrap() == before);
}
