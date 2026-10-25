#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/calculated_storage/fixture.rs"]
mod calculated_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/calculated_memos/fixture.rs"]
mod memo_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/calculated_storage/scalar_fixture.rs"]
mod scalar_fixture;
#[path = "../../../engines/tablecraft-ace/tests/support/mod.rs"]
mod support;

use std::{cell::Cell, io::Cursor};
use tablecraft_access::NativeSource;
use tablecraft_ace::TableSnapshotLimits;
use tablecraft_core::values::Value;
use tablecraft_query::{QueryError, Source};

#[path = "progressive_stored_calculations/harness.rs"]
mod harness;
use harness::*;

#[path = "progressive_stored_calculations/carriers.rs"]
mod carriers;

#[path = "progressive_stored_calculations/boundaries.rs"]
mod boundaries;

#[test]
fn explicit_stored_projections_stream_cached_values_without_retention_or_cache_promotion() {
    for profile in [5, 6] {
        for batch in [1, 2] {
            let mut bytes = calculated_fixture::database(&[Some(999), None, Some(41)]);
            bytes[FORMAT_OFFSET] = profile;
            let original = bytes.clone();
            let source = NativeSource::new_with_snapshot_limits(
                Cursor::new(bytes.as_slice()),
                SCOPE,
                budget(),
                &|| false,
                TableSnapshotLimits::new(1, batch).unwrap(),
            )
            .unwrap()
            .with_stored_calculations()
            .unwrap();
            let expected = vec![
                vec![Value::Int32(1), Value::Int32(999)],
                vec![Value::Int32(2), Value::Null],
                vec![Value::Int32(3), Value::Int32(41)],
            ];
            assert_eq!(collect(&source, &[0, 1]).unwrap(), expected);
            assert_eq!(
                source.is_stored_calculation("Items", 1).unwrap(),
                Some(true)
            );
            assert_eq!(buffered(&source, &[1]), Err(QueryError::StructureLimit));
            assert_eq!(collect(&source, &[0, 1]).unwrap(), expected);
            assert_eq!(buffered(&source, &[0]), Err(QueryError::StructureLimit));
            assert_eq!(collect(&source, &[]).unwrap(), vec![vec![], vec![], vec![]]);
            drop(source);
            assert_eq!(bytes, original);
        }
    }
}

#[test]
fn opt_in_is_required_and_selected_invalid_caches_never_become_partial_snapshots() {
    let mut bytes = calculated_fixture::database(&[Some(999), Some(41), Some(7)]);
    let directory = DATA_PAGE * PAGE_BYTES + ROW_DIRECTORY + 2 * size_of::<u16>();
    let last = usize::from(u16::from_le_bytes(
        bytes[directory..directory + size_of::<u16>()]
            .try_into()
            .unwrap(),
    ));
    bytes[DATA_PAGE * PAGE_BYTES + last + RESULT_OFFSET] = 1;
    let ordinary = source(Cursor::new(bytes.clone()), 1, &|| false, false);
    assert_eq!(collect(&ordinary, &[0]).unwrap().len(), 3);
    assert!(
        ordinary
            .columns("Items")
            .unwrap()
            .iter()
            .all(|column| column.name() != "Score")
    );

    let source = source(Cursor::new(bytes), 1, &|| false, true);
    let mut seen = Vec::new();
    let failure = source.scan_projected_progressive("Items", &[1], &mut |row| {
        seen.push(row.get(0).unwrap().clone());
        Ok(())
    });
    assert_eq!(failure, Err(QueryError::Unsupported));
    assert_eq!(seen, [Value::Int32(999), Value::Int32(41)]);
    assert!(buffered(&source, &[1]).is_err());
    assert_eq!(collect(&source, &[0]).unwrap().len(), 3);
    assert_eq!(collect(&source, &[]).unwrap().len(), 3);
}

#[test]
fn unsupported_calculation_declarations_refuse_even_when_the_table_is_empty() {
    let bytes = scalar_fixture::database(6, 11, 39, false, &[]);
    let source = source(Cursor::new(bytes), 1, &|| false, true);
    assert!(
        source
            .columns("Items")
            .unwrap()
            .iter()
            .all(|column| column.name() != "Score")
    );
    assert!(
        source
            .unavailable_columns("Items")
            .unwrap()
            .iter()
            .any(|name| name.as_str() == "Score")
    );
    assert_eq!(collect(&source, &[0]).unwrap(), Vec::<Vec<Value>>::new());
    assert!(matches!(collect(&source, &[1]), Err(QueryError::Source)));
}

#[test]
fn cancelled_and_rejected_visitors_release_cursors_without_complete_cache_entries() {
    let stopped = Cell::new(false);
    let cancel = || stopped.get();
    let source = source(
        Cursor::new(calculated_fixture::database(&[Some(999), None, Some(41)])),
        1,
        &cancel,
        true,
    );
    let mut visits = 0;
    let failure = source.scan_projected_progressive("Items", &[1], &mut |_| {
        visits += 1;
        stopped.set(true);
        Ok(())
    });
    assert_eq!(failure, Err(QueryError::Cancelled));
    assert_eq!(visits, 1);
    stopped.set(false);
    assert_eq!(collect(&source, &[1]).unwrap().len(), 3);
    assert_eq!(buffered(&source, &[1]), Err(QueryError::StructureLimit));

    visits = 0;
    let failure = source.scan_projected_progressive("Items", &[1], &mut |_| {
        visits += 1;
        Err(QueryError::Parameter)
    });
    assert_eq!(failure, Err(QueryError::Parameter));
    assert_eq!(visits, 1);
    assert_eq!(collect(&source, &[1]).unwrap().len(), 3);
}
