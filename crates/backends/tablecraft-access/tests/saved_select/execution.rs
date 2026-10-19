use super::*;
use tablecraft_access::{NativeSavedSelectError, NativeSource};
use tablecraft_core::values::Value;
use tablecraft_query::{ExecutionOptions, Source, prepare};

#[test]
fn explicitly_loaded_select_executes_against_the_same_native_source_and_parameters() {
    let mut parameter = row(2);
    parameter.name1 = Some("Floor");
    parameter.flag = Some(4);
    let mut table = row(5);
    table.name1 = Some("Items");
    let mut filter = row(8);
    filter.expression = Some("[Id] >= [Floor]");
    let mut sort = row(11);
    sort.expression = Some("[Score]");
    sort.name1 = Some("D");
    let original = database_with_query_flags(
        &[parameter, table, column("[Id]"), filter, sort],
        0x80000000,
    );
    for code in [1, 2, 5, 6] {
        let mut bytes = original.clone();
        bytes[20] = code;
        if code == 1 {
            bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
        }
        let mut source =
            NativeSource::new(Cursor::new(bytes), CatalogScope::new(7), budget(), &|| {
                false
            })
            .unwrap();
        let saved = source
            .read_saved_select("sAvEd", ParseLimits::default())
            .unwrap();
        assert_eq!(saved.definition().origin().native_id(), QUERY_ID);
        assert_eq!(saved.definition().rows()[0].name1(), Some("Floor"));
        let plan = prepare(saved.query(), &source).unwrap();
        let result = plan
            .execute(
                &source,
                &[("Floor", Value::Int32(2))],
                ExecutionOptions::default(),
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            [vec![Value::Int32(3)], vec![Value::Int32(2)]]
        );
        assert!(
            plan.execute(&source, &[], ExecutionOptions::default())
                .is_err()
        );
    }
}

#[test]
fn saved_metadata_and_table_reads_share_budget_and_cancellation() {
    let bytes = database_with_query_flags(&[column("1")], 0x80000000);
    let cancelled = Cell::new(false);
    let cancel = || cancelled.get();
    let mut source = NativeSource::new(
        Cursor::new(bytes.clone()),
        CatalogScope::new(7),
        budget(),
        &cancel,
    )
    .unwrap();
    cancelled.set(true);
    assert!(matches!(
        source.read_saved_select("Saved", ParseLimits::default()),
        Err(NativeSavedSelectError::Native(
            tablecraft_ace::InspectError::Cancelled
        ))
    ));
    cancelled.set(false);
    assert!(
        source
            .read_saved_select("Saved", ParseLimits::default())
            .is_ok()
    );
    assert!(
        source
            .read_saved_select("Missing", ParseLimits::default())
            .is_err()
    );
    let mut limited = NativeSource::new(
        Cursor::new(bytes),
        CatalogScope::new(7),
        ResourceBudget::new(64 * 1024 * 1024, 100, 1_000_000),
        &|| false,
    )
    .unwrap();
    let mut succeeded = 0;
    loop {
        match limited.read_saved_select("Saved", ParseLimits::default()) {
            Ok(_) => {
                succeeded += 1;
                assert!(succeeded < 100);
            }
            Err(NativeSavedSelectError::Native(tablecraft_ace::InspectError::Limit(_)))
            | Err(NativeSavedSelectError::Reconstruction(E::Limit(_))) => break,
            Err(error) => panic!("unexpected failure: {error}"),
        }
    }
    assert!(succeeded > 0);
    assert!(limited.columns("Items").is_err());
}
