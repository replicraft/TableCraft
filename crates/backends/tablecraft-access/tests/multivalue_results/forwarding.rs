use super::*;
use tablecraft_query::source::SourceKey;

#[test]
fn authorized_link_and_external_targets_forward_exact_typed_parent_lists() {
    let pool = RefCell::new(budget());
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(linked_fixture::database(
            6,
            Some("opaque"),
            Some("Items"),
            None,
        )),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap();
    let target = NativeSource::new_with_shared_budget(
        Cursor::new(populated(6)),
        CatalogScope::new(2),
        &pool,
        &|| false,
    )
    .unwrap()
    .with_multivalues();
    let links = [NativeLinkMapping::new(
        "Linked",
        -1,
        "opaque",
        target.handle(),
        "Items",
        10,
    )];
    let linked = LinkedNativeSource::new(root.handle(), &links).unwrap();
    let external = ExternalNativeSource::new(
        root.handle(),
        &[NativeExternalMapping::new(
            "authorized",
            SourceKey::new(7),
            target.handle(),
        )],
    )
    .unwrap();
    for (source, sql) in [
        (&linked as &dyn Source, "SELECT Id,Score FROM Linked"),
        (
            &external as &dyn Source,
            "SELECT Id,Score FROM Items IN \"authorized\"",
        ),
    ] {
        assert_eq!(
            observe(&query(source, sql).unwrap()),
            [
                (1, vec![Value::Int32(100), Value::Int32(101)]),
                (2, vec![Value::Int32(200)]),
                (3, vec![]),
            ]
        );
    }
}
