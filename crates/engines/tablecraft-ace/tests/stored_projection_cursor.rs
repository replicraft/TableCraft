#[path = "calculated_storage/fixture.rs"]
mod calculated_fixture;
#[path = "mutation/fixture.rs"]
mod fixture;
use tablecraft_testkit::access as support;

use std::{
    cell::Cell,
    io::{Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_ace::{InspectError, NativeReadSource, TableCursor, TableSnapshotLimits};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 16384, 1_000_000)
}

struct Watching {
    bytes: Cursor<Vec<u8>>,
    forbidden: Rc<Cell<bool>>,
}
impl NativeReadSource for Watching {}
impl Read for Watching {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if self.bytes.position() as usize / 4096 == 12 {
            assert!(
                !self.forbidden.get(),
                "stored cursor opening read user records"
            );
        }
        self.bytes.read(bytes)
    }
}
impl Seek for Watching {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.bytes.seek(position)
    }
}

#[test]
fn stored_projection_opening_is_inert_and_complete_batches_preserve_native_order() {
    let forbidden = Rc::new(Cell::new(true));
    let mut source = Watching {
        bytes: Cursor::new(calculated_fixture::database(&[Some(999), None, Some(41)])),
        forbidden: forbidden.clone(),
    };
    let mut quota = budget();
    let cursor = TableCursor::open_stored_projection(
        &mut source,
        CatalogScope::new(1),
        "Items",
        &["Score", "Id"],
        &mut quota,
        &|| false,
    )
    .unwrap();
    assert_eq!(
        cursor
            .columns()
            .iter()
            .map(|column| column.name())
            .collect::<Vec<_>>(),
        ["Id", "Score"]
    );
    assert!(cursor.columns()[1].nullable());
    forbidden.set(false);
    let snapshot = cursor
        .into_snapshot(
            TableSnapshotLimits::new(10, 1).unwrap(),
            &mut quota,
            &|| false,
        )
        .unwrap();
    assert_eq!(
        snapshot.rows(),
        [
            vec![Value::Int32(1), Value::Int32(999)],
            vec![Value::Int32(2), Value::Null],
            vec![Value::Int32(3), Value::Int32(41)],
        ]
    );
}

#[test]
fn a_late_failure_closes_continuation_and_cannot_publish_a_complete_snapshot() {
    let mut bytes = calculated_fixture::database(&[Some(20), Some(41)]);
    let second =
        u16::from_le_bytes(bytes[12 * 4096 + 16..12 * 4096 + 18].try_into().unwrap()) as usize;
    bytes[12 * 4096 + second + 6] = 1;
    let mut source = Cursor::new(bytes.clone());
    let mut quota = budget();
    let mut cursor = TableCursor::open_stored_projection(
        &mut source,
        CatalogScope::new(1),
        "Items",
        &["Score"],
        &mut quota,
        &|| false,
    )
    .unwrap();
    assert_eq!(
        cursor.next_page(1, &mut quota, &|| false).unwrap().rows(),
        [vec![Value::Int32(20)]]
    );
    assert_eq!(
        cursor.next_page(1, &mut quota, &|| false).err(),
        Some(InspectError::Unsupported)
    );
    assert!(cursor.is_failed());
    assert_eq!(
        cursor.next_page(1, &mut quota, &|| false).err(),
        Some(InspectError::CursorFailed)
    );
    drop(cursor);
    let cursor = TableCursor::open_stored_projection(
        &mut source,
        CatalogScope::new(1),
        "Items",
        &["Score"],
        &mut quota,
        &|| false,
    )
    .unwrap();
    assert!(
        cursor
            .into_snapshot(
                TableSnapshotLimits::new(10, 1).unwrap(),
                &mut quota,
                &|| false
            )
            .is_err()
    );
    assert_eq!(source.into_inner(), bytes);
}

#[test]
fn zero_width_and_ordinary_projections_prune_caches_but_selected_declarations_still_qualify() {
    let mut bytes = calculated_fixture::database(&[Some(20), Some(41)]);
    bytes[10 * 4096 + 63 + 25 + 23] = 38;
    let mut source = Cursor::new(bytes);
    let mut quota = budget();
    assert!(
        TableCursor::open_stored_projection(
            &mut source,
            CatalogScope::new(1),
            "Items",
            &["Score"],
            &mut quota,
            &|| false
        )
        .is_err()
    );
    for names in [&[][..], &["Id"][..]] {
        let cursor = TableCursor::open_stored_projection(
            &mut source,
            CatalogScope::new(1),
            "Items",
            names,
            &mut quota,
            &|| false,
        )
        .unwrap();
        assert_eq!(
            cursor
                .into_snapshot(
                    TableSnapshotLimits::new(10, 1).unwrap(),
                    &mut quota,
                    &|| false
                )
                .unwrap()
                .rows()
                .len(),
            2
        );
    }
    for names in [&["Id", "Id"][..], &["Unknown"][..]] {
        assert!(
            TableCursor::open_stored_projection(
                &mut source,
                CatalogScope::new(1),
                "Items",
                names,
                &mut quota,
                &|| false
            )
            .is_err()
        );
    }
    assert!(
        TableCursor::open_projection(
            &mut source,
            CatalogScope::new(1),
            "Items",
            &["Score"],
            &mut quota,
            &|| false
        )
        .is_err()
    );
}
