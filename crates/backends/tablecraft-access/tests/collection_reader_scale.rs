//! Actual source-owned browser consumers over the retained ACE16 scale files.
use sha2::{Digest, Sha256};
use std::{io::Cursor, sync::OnceLock};
use tablecraft_access::NativeBrowser;
use tablecraft_ace::{InspectError, MultivalueReader, TableSnapshotLimits};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::collection_scale::{self as model, Sources},
    support::ace::collection_scale as observe,
};

const COLLECTION_SCOPE: CatalogScope = CatalogScope::new(0x51);
const STORED_SCOPE: CatalogScope = CatalogScope::new(0x52);

fn sources() -> &'static Sources {
    static SOURCES: OnceLock<Sources> = OnceLock::new();
    SOURCES.get_or_init(|| {
        model::load(|bytes, expected| {
            let hash: String = Sha256::digest(bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            assert_eq!(hash, expected);
        })
    })
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 100_000_000)
}

#[test]
#[ignore = "requires registered ACE16 collection-scale originals"]
fn actual_browser_reads_sparse_and_dense_multivalues_from_their_own_original_namespace() {
    let mut browser = NativeBrowser::new(
        Cursor::new(&sources().collections),
        COLLECTION_SCOPE,
        budget(),
        &|| false,
    );
    {
        let mut reader = browser
            .multivalue_reader(
                "ManyParents",
                "Numbers",
                TableSnapshotLimits::new(48_000, 256).unwrap(),
            )
            .unwrap();
        assert_eq!(reader.schema().owner().name(), "ManyParents");
        assert_eq!(reader.parents().len(), 20_000);
        assert_eq!(
            reader
                .parents()
                .iter()
                .map(|parent| parent.child_count())
                .sum::<usize>(),
            48_000
        );
        for parent in [0, 4, 19_998, 19_999] {
            observe::assert_many_parent(&reader.read_parent(parent).unwrap(), parent);
        }
    }
    // A different table/flat child directory uses the same browser allowance,
    // not the preceding table's membership or a new hidden budget.
    let mut reader = browser
        .multivalue_reader(
            "ManyChildren",
            "Numbers",
            TableSnapshotLimits::new(40_000, 256).unwrap(),
        )
        .unwrap();
    assert_eq!(reader.schema().owner().name(), "ManyChildren");
    assert_eq!(reader.parents().len(), 400);
    observe::assert_dense_parent(&reader.read_parent(399).unwrap());
}

#[test]
#[ignore = "requires registered ACE16 collection-scale originals"]
fn actual_browser_keeps_encoded_files_and_stored_history_as_distinct_typed_views() {
    let mut browser = NativeBrowser::new(
        Cursor::new(&sources().stored),
        STORED_SCOPE,
        budget(),
        &|| false,
    );
    let files;
    {
        let mut reader = browser
            .attachment_reader(
                "ManyFiles",
                "Files",
                TableSnapshotLimits::new(20_000, 256).unwrap(),
            )
            .unwrap();
        assert_eq!(reader.schema().owner().name(), "ManyFiles");
        assert_eq!(reader.parents().len(), 201);
        files = reader.read_parent(199).unwrap();
        observe::assert_files(&files, 199);
        observe::assert_files(&reader.read_parent(200).unwrap(), 200);
    }
    {
        let mut reader = browser
            .version_history_reader(
                "Histories",
                "Notes",
                TableSnapshotLimits::new(20_000, 256).unwrap(),
            )
            .unwrap();
        assert_eq!(reader.schema().owner().name(), "Histories");
        assert_eq!(reader.memo_column().name(), "Notes");
        assert_eq!(reader.parents().len(), 201);
        for parent in [0, 100, 199, 200] {
            observe::assert_history(&reader.read_parent(parent).unwrap(), parent);
        }
    }
    // The first typed result stays owned through another reader on the same
    // source; history never replaces its encoded payloads or metadata.
    observe::assert_files(&files, 199);
}

#[test]
#[ignore = "requires registered ACE16 collection-scale originals"]
fn actual_browser_cannot_refill_a_large_reader_budget_when_the_next_parent_is_empty() {
    let bytes = &sources().collections;
    let limits = TableSnapshotLimits::new(48_000, 256).unwrap();
    let mut measured = budget();
    {
        let mut source = Cursor::new(bytes);
        let mut reader = MultivalueReader::open(
            &mut source,
            COLLECTION_SCOPE,
            "ManyParents",
            "Numbers",
            limits,
            &mut measured,
            &|| false,
        )
        .unwrap();
        observe::assert_many_parent(&reader.read_parent(0).unwrap(), 0);
    }
    let rows = 1_000_000 - measured.remaining(Resource::Rows);
    let result;
    {
        let mut browser = NativeBrowser::new(
            Cursor::new(bytes),
            COLLECTION_SCOPE,
            ResourceBudget::new(256 * 1024 * 1024, rows, 100_000_000),
            &|| false,
        );
        let mut reader = browser
            .multivalue_reader("ManyParents", "Numbers", limits)
            .unwrap();
        result = reader.read_parent(0).unwrap();
        observe::assert_many_parent(&result, 0);
        let error = reader
            .read_parent(4)
            .err()
            .expect("empty output is not a complete-census shortcut");
        assert!(matches!(error, InspectError::Limit(limit) if limit.resource == Resource::Rows));
        assert!(reader.is_failed());
        assert!(matches!(
            reader.read_parent(0),
            Err(InspectError::CollectionReaderFailed)
        ));
    }
    observe::assert_many_parent(&result, 0);
}
