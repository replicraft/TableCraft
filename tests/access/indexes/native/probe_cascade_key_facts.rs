#![cfg(feature = "ace16-write-preview")]

use std::{
    collections::BTreeSet,
    fs,
    io::{self, Cursor, Read, Seek, SeekFrom},
    path::Path,
};
use tablecraft_ace::{
    NativeImage, NativeReadSource, RowDelete, TableRowDeletes, read_table, read_table_projection,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

const PAGE_BYTES: u64 = 4096;
const BYTES: u64 = 256 * 1024 * 1024;
const ROWS: u64 = 100_000;
const STEPS: u64 = 100_000_000;

fn budget() -> ResourceBudget {
    ResourceBudget::new(BYTES, ROWS, STEPS)
}

struct Reads {
    cursor: Cursor<Vec<u8>>,
    cold: BTreeSet<u64>,
    reads: usize,
}

impl Read for Reads {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let start = self.cursor.position();
        if start.is_multiple_of(PAGE_BYTES)
            && bytes.len() as u64 == PAGE_BYTES
            && self.cold.contains(&(start / PAGE_BYTES))
        {
            self.reads += 1;
        }
        self.cursor.read(bytes)
    }
}

impl Seek for Reads {
    fn seek(&mut self, at: SeekFrom) -> io::Result<u64> {
        self.cursor.seek(at)
    }
}

impl NativeReadSource for Reads {}

fn requests(path: &Path) -> Vec<RowDelete> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| {
            let (row, encoded) = line.split_once('\t').unwrap();
            let raw = encoded
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            RowDelete::new(row.parse().unwrap(), raw)
        })
        .collect()
}

#[test]
fn native_key_facts_metrics_probe() {
    let root =
        std::env::var("TABLECRAFT_FACTS_CORPUS").expect("Explicit private corpus path required");
    let root = Path::new(&root);
    for profile in ["V2016", "V2019"] {
        let raw = fs::read(root.join(format!("KeyFacts-{profile}-before.accdb"))).unwrap();
        let cold: BTreeSet<u64> =
            fs::read_to_string(root.join(format!("cold-pages-{profile}.txt")))
                .unwrap()
                .lines()
                .map(|value| value.parse().unwrap())
                .collect();
        for (table, names, rows) in [
            ("Parents", &["Tenant", "Id"][..], 3),
            ("Children", &["TenantFk", "ParentId"][..], 6),
        ] {
            for projected in [false, true] {
                let mut source = Reads {
                    cursor: Cursor::new(raw.clone()),
                    cold: cold.clone(),
                    reads: 0,
                };
                let mut pool = budget();
                let snapshot = if projected {
                    read_table_projection(
                        &mut source,
                        CatalogScope::new(1),
                        table,
                        names,
                        &mut pool,
                        &|| false,
                    )
                } else {
                    read_table(&mut source, CatalogScope::new(1), table, &mut pool, &|| {
                        false
                    })
                }
                .unwrap();
                assert_eq!(snapshot.rows().len(), rows);
                if projected {
                    assert_eq!(source.reads, 0);
                } else {
                    assert!(source.reads > 0);
                }
                println!(
                    "SNAPSHOT profile={profile} table={table} projected={projected} cold_page_reads={} bytes={} rows={} steps={}",
                    source.reads,
                    BYTES - pool.remaining(Resource::Bytes),
                    ROWS - pool.remaining(Resource::Rows),
                    STEPS - pool.remaining(Resource::Steps)
                );
            }
        }
        for (label, table, expected) in [
            ("first-parent", "Parents", 3),
            ("all-parents", "Parents", 8),
            ("child-only", "Children", 1),
        ] {
            let deletes =
                requests(&root.join(format!("before-candidates-v2/{profile}-{label}.tsv")));
            let image = NativeImage::capture(
                &mut Cursor::new(&raw),
                CatalogScope::new(1),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let mut pool = budget();
            let candidate = image
                .preview_ace16_cascading_deletes(
                    &[TableRowDeletes::new(table, &deletes)],
                    &mut pool,
                    &|| false,
                )
                .unwrap();
            assert_eq!(candidate.deleted_rows(), expected);
            let baseline =
                fs::read(root.join(format!("before-candidates-v2/{profile}-{label}.accdb")))
                    .unwrap();
            assert_eq!(candidate.candidate_bytes(), baseline);
            println!(
                "OPERATION profile={profile} case={label} bytes={} rows={} steps={}",
                BYTES - pool.remaining(Resource::Bytes),
                ROWS - pool.remaining(Resource::Rows),
                STEPS - pool.remaining(Resource::Steps)
            );
        }
    }
}

#[test]
fn malformed_unselected_long_values_still_block_complete_cascade_preflight() {
    let root = std::env::var("TABLECRAFT_FACTS_CORPUS").unwrap();
    let root = Path::new(&root);
    for profile in ["V2016", "V2019"] {
        let mut raw = fs::read(root.join(format!("KeyFacts-{profile}-before.accdb"))).unwrap();
        let cold = fs::read_to_string(root.join(format!("cold-pages-{profile}.txt"))).unwrap();
        for page in cold.lines().map(|line| line.parse::<usize>().unwrap()) {
            let owner = page * 4096 + 4;
            assert_eq!(&raw[owner..owner + 4], b"LVAL");
            raw[owner] ^= 0x80;
        }
        for (table, names) in [
            ("Parents", &["Tenant", "Id"][..]),
            ("Children", &["TenantFk", "ParentId"][..]),
        ] {
            read_table_projection(
                &mut Cursor::new(&raw),
                CatalogScope::new(1),
                table,
                names,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert!(matches!(
                read_table(
                    &mut Cursor::new(&raw),
                    CatalogScope::new(1),
                    table,
                    &mut budget(),
                    &|| false
                ),
                Err(tablecraft_ace::InspectError::Malformed),
            ));
        }
        let deletes =
            requests(&root.join(format!("before-candidates-v2/{profile}-first-parent.tsv")));
        let image = NativeImage::capture(
            &mut Cursor::new(&raw),
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let result = image.preview_ace16_cascading_deletes(
            &[TableRowDeletes::new("Parents", &deletes)],
            &mut budget(),
            &|| false,
        );
        assert!(matches!(
            result,
            Err(tablecraft_ace::EditError::Inspect(
                tablecraft_ace::InspectError::Malformed
            ))
        ));
        println!(
            "CORRUPTION profile={profile} key_projection=success complete_reads=Malformed cascade_preflight=Malformed"
        );
    }
}
