use super::*;
use saved_fixture::{Fragment, QUERY_ID};

fn row(attribute: u8) -> Fragment<'static> {
    Fragment {
        object: Some(QUERY_ID),
        attribute: Some(attribute),
        order: Some(&[0, 0, 0, 1]),
        ..Default::default()
    }
}

fn image(profile: u8, expression: &str, distinct: bool) -> Vec<u8> {
    let mut bytes = saved_fixture::database_with_query_flags(
        &[
            Fragment {
                name1: Some("Items"),
                ..row(5)
            },
            Fragment {
                expression: Some(expression),
                name1: Some("Chosen"),
                ..row(6)
            },
        ],
        0x8000_0000,
    );
    if distinct {
        let target = distinct_target_bytes();
        let start = ROW_PAGE as usize * PAGE_BYTES as usize;
        let end = start + PAGE_BYTES as usize;
        bytes[start..end].copy_from_slice(&target[start..end]);
    }
    bytes[20] = profile;
    if profile == 1 {
        bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
    }
    bytes
}

#[test]
fn external_saved_reads_bind_metadata_without_rows_and_keep_target_ownership() {
    for profile in [1, 2, 5, 6] {
        let pool = RefCell::new(allowance());
        let forbidden = Cell::new(true);
        let failed = Cell::new(false);
        let root = NativeSource::new_with_shared_budget(
            Cursor::new(image(profile, "Score", false)),
            CatalogScope::new(1),
            &pool,
            &|| false,
        )
        .unwrap();
        let target = NativeSource::new_with_shared_budget(
            Watching {
                inner: Cursor::new(image(profile, "Score", true)),
                forbid_rows: &forbidden,
                fail_rows: &failed,
            },
            CatalogScope::new(2),
            &pool,
            &|| false,
        )
        .unwrap();
        let facade = ExternalNativeSource::new(root.handle(), &[mapping(&target)]).unwrap();
        let parsed =
            parse_query("SELECT Chosen FROM Saved IN 'remote'", Default::default()).unwrap();
        let plan = prepare(parsed.query(), &facade).unwrap();
        forbidden.set(false);
        let result = plan.execute(&facade, &[], Default::default()).unwrap();
        assert_eq!(
            result.rows(),
            [
                vec![Value::Int32(111)],
                vec![Value::Int32(222)],
                vec![Value::Int32(333)]
            ]
        );
        assert_ne!(
            scores(&facade, "SELECT Chosen FROM Saved").unwrap(),
            result.rows()
        );
    }
}

#[test]
fn external_saved_read_failures_do_not_fall_back_to_root_definitions() {
    let pool = RefCell::new(allowance());
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(image(6, "Score", false)),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap();
    let target = NativeSource::new_with_shared_budget(
        Cursor::new(image(6, "NeverRunMe(Score)", true)),
        CatalogScope::new(2),
        &pool,
        &|| false,
    )
    .unwrap();
    let facade = ExternalNativeSource::new(root.handle(), &[mapping(&target)]).unwrap();
    assert!(scores(&facade, "SELECT Chosen FROM Saved").is_ok());
    assert!(matches!(
        scores(&facade, "SELECT Chosen FROM Saved IN 'remote'"),
        Err(QueryError::Unsupported)
    ));
    assert!(matches!(
        scores(&facade, "SELECT * FROM Missing IN 'remote'"),
        Err(QueryError::UnknownName)
    ));
    let remote = facade.external_source(request("remote")).unwrap().unwrap();
    assert!(remote.source().query("Saved").unwrap().is_some());
    assert!(
        remote
            .source()
            .external_source(request("other"))
            .unwrap()
            .is_none()
    );
}
