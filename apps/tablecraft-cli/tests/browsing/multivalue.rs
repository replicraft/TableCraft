use super::{FileCopy, calculated_fixture, fixture as base, success, support};

#[path = "../../../../crates/engines/tablecraft-ace/tests/complex_schema/fixture.rs"]
mod complex_fixture;
#[path = "../../../../crates/engines/tablecraft-ace/tests/complex_links/fixture.rs"]
mod links_fixture;

#[test]
fn mixed_multivalue_browsing_is_explicit_and_keeps_null_empty_and_paging_distinctions() {
    let mut bytes = links_fixture::populated();
    const PAGE_BYTES: usize = 4096;
    const PARENT_DATA_PAGE: usize = 12;
    const THIRD_SLOT: usize = 18;
    const NULL_MASK_OFFSET: usize = 10;
    let row = u16::from_le_bytes(
        bytes[PARENT_DATA_PAGE * PAGE_BYTES + THIRD_SLOT
            ..PARENT_DATA_PAGE * PAGE_BYTES + THIRD_SLOT + 2]
            .try_into()
            .unwrap(),
    ) as usize;
    bytes[PARENT_DATA_PAGE * PAGE_BYTES + row + NULL_MASK_OFFSET] = 1;
    let file = FileCopy::new(bytes);
    let ordinary = file.run("rows", &["Items"]);
    assert_eq!(ordinary.status.code(), Some(2));
    assert!(ordinary.stdout.is_empty());
    assert_eq!(
        success(file.run("rows", &["Items", "--multivalues", "--format", "tsv"])),
        b"Id\tScore [multivalue]\n1\t[100, 101]\n2\t[200]\n3\tNULL\n"
    );
    assert_eq!(
        success(file.run(
            "rows",
            &["Items", "--multivalues", "--offset", "1", "--limit", "1"]
        )),
        b"Id\tScore [multivalue]\n2\t[200]\n"
    );
    assert_eq!(
        success(file.run(
            "rows",
            &["Items", "--multivalues", "--offset", "4294967295"]
        )),
        b"Id\tScore [multivalue]\n"
    );
    let pretty = String::from_utf8(success(file.run(
        "rows",
        &[
            "Items",
            "--multivalues",
            "--format",
            "table",
            "--limit",
            "1",
        ],
    )))
    .unwrap();
    assert!(pretty.starts_with('┌'));
    assert!(pretty.contains("[100, 101]"));
    // Cached-only tables can use the mixed profile only with the separate cache opt-in.
    let cache = FileCopy::new(calculated_fixture::database(&[Some(41)]));
    let result = cache.run("rows", &["Items", "--multivalues"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert_eq!(
        success(cache.run("rows", &["Items", "--multivalues", "--stored-calculations"])),
        b"Id\tScore [cached]\n1\t41\n"
    );
}

#[test]
fn all_collection_mode_retains_multivalue_contents_and_rejects_conflicting_opt_ins() {
    let file = FileCopy::new(links_fixture::populated());
    let restricted = success(file.run("rows", &["Items", "--multivalues", "--format", "tsv"]));
    let general = success(file.run("rows", &["Items", "--collections", "--format", "tsv"]));
    assert_eq!(general, restricted);
    let result = file.run("rows", &["Items", "--collections", "--multivalues"]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
}
