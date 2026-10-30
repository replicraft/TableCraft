use tablecraft_testkit::access::mutation_fixture as fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/index_fixture.rs"]
mod index_fixture;
use std::{
    cell::Cell,
    io::{Cursor, Read, Seek, SeekFrom},
    ops::Bound::Included,
    rc::Rc,
};
use tablecraft_access::NativeSource;
use tablecraft_ace::TableSnapshotLimits;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, QueryError, Source, prepare, source::ScanConstraint};
use tablecraft_sql::parse_query;
use tablecraft_testkit::access as support;

struct Reads {
    input: Cursor<Vec<u8>>,
    data: Rc<Cell<usize>>,
}
impl tablecraft_ace::NativeReadSource for Reads {}
impl Read for Reads {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if self.input.position() == 12 * 4096 {
            self.data.set(self.data.get() + 1);
        }
        self.input.read(bytes)
    }
}
impl Seek for Reads {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.input.seek(position)
    }
}
fn new_source(bytes: Vec<u8>, rows: u32, data: Rc<Cell<usize>>) -> NativeSource<'static, Reads> {
    NativeSource::new_with_snapshot_limits(
        Reads {
            input: Cursor::new(bytes),
            data,
        },
        CatalogScope::new(1),
        ResourceBudget::new(32 * 1024 * 1024, 10000, 1_000_000),
        &|| false,
        TableSnapshotLimits::new(rows, 1).unwrap(),
    )
    .unwrap()
}

#[test]
fn caller_limits_keep_binding_metadata_only_and_complete_snapshots_reentrant() {
    let data = Rc::new(Cell::new(0));
    let source = new_source(fixture::database(), 3, data.clone());
    let parsed = parse_query(
        "SELECT Id,(SELECT Count(*) FROM Items) FROM Items ORDER BY Id",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(data.get(), 0);
    assert_eq!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        &[
            vec![Value::Int32(1), Value::Int64(3)],
            vec![Value::Int32(2), Value::Int64(3)],
            vec![Value::Int32(3), Value::Int64(3)],
        ]
    );
    assert_eq!(data.get(), 1);
    let mut visits = 0;
    source
        .scan("Items", &mut |_| {
            let mut inner = 0;
            source.scan("Items", &mut |_| {
                inner += 1;
                Ok(())
            })?;
            assert_eq!(inner, 3);
            visits += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(visits, 3);
    assert_eq!(
        data.get(),
        2,
        "full data is loaded once after the Id-only projection"
    );
}

#[test]
fn lower_caps_and_late_failures_never_publish_visitors_or_cache_partial_rows() {
    let reads = Rc::new(Cell::new(0));
    let source = new_source(fixture::database(), 2, reads.clone());
    for _ in 0..2 {
        assert_eq!(
            source.scan("Items", &mut |_| panic!(
                "no partial snapshot may reach a visitor"
            )),
            Err(QueryError::StructureLimit)
        );
    }
    assert_eq!(reads.get(), 0);
    let mut bytes = fixture::database();
    let slot = 12 * 4096 + 18;
    let row = u16::from_le_bytes(bytes[slot..slot + 2].try_into().unwrap()) as usize & 0x1fff;
    bytes[12 * 4096 + row..12 * 4096 + row + 2].copy_from_slice(&100_u16.to_le_bytes());
    let reads = Rc::new(Cell::new(0));
    let broken = new_source(bytes, 3, reads.clone());
    for count in 1..=2 {
        assert!(
            broken
                .scan("Items", &mut |_| panic!(
                    "late failure must precede every visitor"
                ))
                .is_err()
        );
        assert_eq!(reads.get(), count);
    }
}

#[test]
fn scalar_retention_limits_also_cover_complete_and_partial_selected_indexes() {
    let source = new_source(
        index_fixture::database(false, false, true),
        1,
        Rc::new(Cell::new(0)),
    )
    .with_index("Items", "ScoreKey")
    .unwrap();
    let pair = ScanConstraint::Int32 {
        column: 1,
        lower: Included(20),
        upper: Included(30),
    };
    assert_eq!(
        source.scan_constrained("Items", pair, &mut |_| panic!(
            "retained rows exceed the explicit cap"
        )),
        Err(QueryError::StructureLimit)
    );
    assert_eq!(
        source.scan_projected("Items", &[0], Some(pair), &mut |_| panic!(
            "projected selected snapshots have the same cap"
        )),
        Err(QueryError::StructureLimit)
    );
    let point = ScanConstraint::Int32 {
        column: 1,
        lower: Included(20),
        upper: Included(20),
    };
    let mut rows = 0;
    source
        .scan_constrained("Items", point, &mut |_| {
            rows += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(rows, 1);
    assert_eq!(
        source.scan("Items", &mut |_| panic!(
            "a partial index selection cannot become a complete table"
        )),
        Err(QueryError::StructureLimit)
    );
}
