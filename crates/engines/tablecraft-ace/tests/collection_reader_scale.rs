//! Selected-parent scale acceptance over unchanged independently qualified files.
use sha2::{Digest, Sha256};
use std::{io::Cursor, sync::OnceLock};
use tablecraft_ace::{
    AttachmentDecoder, AttachmentReader, InspectError, MultivalueReader, TableSnapshotLimits,
    VersionHistoryReader, read_attachment_column_with_limits, read_multivalue_column_with_limits,
    read_version_history_column_with_limits,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::collection_scale::{self as model, Sources},
    support::ace::collection_scale as observe,
};

const COLLECTION_SCOPE: CatalogScope = CatalogScope::new(0x51);
const STORED_SCOPE: CatalogScope = CatalogScope::new(0x52);
const MANY_PARENTS: [usize; 4] = [0, 4, 19_998, 19_999];
const STORED_PARENTS: [usize; 4] = [0, 100, 199, 200];

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
fn twenty_thousand_parent_directory_matches_recipe_and_complete_multivalue_reader() {
    let bytes = &sources().collections;
    let limits = TableSnapshotLimits::new(48_000, 256).unwrap();
    let full = read_multivalue_column_with_limits(
        &mut Cursor::new(bytes),
        COLLECTION_SCOPE,
        "ManyParents",
        "Numbers",
        limits,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut source = Cursor::new(bytes);
    let mut allowance = budget();
    let mut reader = MultivalueReader::open(
        &mut source,
        COLLECTION_SCOPE,
        "ManyParents",
        "Numbers",
        limits,
        &mut allowance,
        &|| false,
    )
    .unwrap();
    assert_eq!(reader.parents().len(), 20_000);
    assert_eq!(
        reader
            .parents()
            .iter()
            .map(|parent| parent.child_count())
            .sum::<usize>(),
        48_000
    );
    for (ordinal, parent) in reader.parents().iter().enumerate() {
        assert_eq!(
            parent.child_count(),
            if (ordinal + 1) % 5 == 0 { 0 } else { 3 }
        );
    }
    for parent in MANY_PARENTS {
        let selected = reader.read_parent(parent).unwrap();
        observe::assert_many_parent(&selected, parent);
        assert!(selected.row_reference() == full.links().parents()[parent].row_reference());
        assert_eq!(selected.key(), full.links().parents()[parent].key());
        assert_eq!(
            selected.values().iter().collect::<Vec<_>>(),
            full.values(parent).unwrap().collect::<Vec<_>>()
        );
    }
    drop(reader);
    assert!(allowance.remaining(Resource::Rows) <= 1_000_000 - 5 * 68_000);
}

#[test]
#[ignore = "requires registered ACE16 collection-scale originals"]
fn forty_thousand_dense_children_keep_full_caps_and_the_separate_hundred_value_policy() {
    let bytes = &sources().collections;
    let limits = TableSnapshotLimits::new(40_000, 1).unwrap();
    let mut source = Cursor::new(bytes);
    let mut allowance = budget();
    let mut reader = MultivalueReader::open(
        &mut source,
        COLLECTION_SCOPE,
        "ManyChildren",
        "Numbers",
        limits,
        &mut allowance,
        &|| false,
    )
    .unwrap();
    assert_eq!(reader.parents().len(), 400);
    assert!(
        reader
            .parents()
            .iter()
            .all(|parent| parent.child_count() == 100)
    );
    for parent in [0, 399] {
        observe::assert_dense_parent(&reader.read_parent(parent).unwrap());
    }
    drop(reader);
    for (table, limit) in [
        ("ManyParents", TableSnapshotLimits::default()),
        (
            "ManyChildren",
            TableSnapshotLimits::new(39_999, 256).unwrap(),
        ),
        ("Overflow", TableSnapshotLimits::new(48_000, 256).unwrap()),
    ] {
        let mut source = Cursor::new(bytes);
        let mut allowance = budget();
        assert!(matches!(
            MultivalueReader::open(
                &mut source,
                COLLECTION_SCOPE,
                table,
                "Numbers",
                limit,
                &mut allowance,
                &|| false,
            ),
            Err(InspectError::StructureLimit)
        ));
    }
}

#[test]
#[ignore = "requires registered ACE16 collection-scale originals"]
fn twenty_thousand_attachment_directory_preserves_encoded_bytes_and_explicit_selected_files() {
    let bytes = &sources().stored;
    let limits = TableSnapshotLimits::new(20_000, 256).unwrap();
    let full = read_attachment_column_with_limits(
        &mut Cursor::new(bytes),
        STORED_SCOPE,
        "ManyFiles",
        "Files",
        limits,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut source = Cursor::new(bytes);
    let mut allowance = budget();
    let mut reader = AttachmentReader::open(
        &mut source,
        STORED_SCOPE,
        "ManyFiles",
        "Files",
        limits,
        &mut allowance,
        &|| false,
    )
    .unwrap();
    assert_eq!(reader.parents().len(), 201);
    assert_eq!(
        reader
            .parents()
            .iter()
            .map(|parent| parent.child_count())
            .sum::<usize>(),
        20_000
    );
    let mut decoded_budget = ResourceBudget::new(4 * 1024 * 1024, 1_000, 10_000_000);
    let mut decoder = AttachmentDecoder::new(&mut decoded_budget, &|| false);
    for parent in STORED_PARENTS {
        let selected = reader.read_parent(parent).unwrap();
        observe::assert_files(&selected, parent);
        assert!(selected.row_reference() == full.links().parents()[parent].row_reference());
        assert_eq!(selected.key(), full.links().parents()[parent].key());
        assert_eq!(
            full.attachments(parent).unwrap().len(),
            selected.attachments().len()
        );
        for (stored, original) in selected
            .attachments()
            .zip(full.attachments(parent).unwrap())
        {
            let (stored, original) = (stored.unwrap(), original.unwrap());
            assert_eq!(stored.identity(), original.identity());
            assert_eq!(stored.encoded_file_data(), original.encoded_file_data());
        }
        observe::assert_decoded_files(&selected, parent, &mut decoder);
    }
    drop(reader);
    let mut source = Cursor::new(bytes);
    let mut allowance = budget();
    assert!(matches!(
        AttachmentReader::open(
            &mut source,
            STORED_SCOPE,
            "ManyFiles",
            "Files",
            TableSnapshotLimits::default(),
            &mut allowance,
            &|| false,
        ),
        Err(InspectError::StructureLimit)
    ));
}

#[test]
#[ignore = "requires registered ACE16 collection-scale originals"]
fn twenty_thousand_stored_versions_keep_recipe_order_dates_and_complete_reader_identity() {
    let bytes = &sources().stored;
    let limits = TableSnapshotLimits::new(20_000, 256).unwrap();
    let full = read_version_history_column_with_limits(
        &mut Cursor::new(bytes),
        STORED_SCOPE,
        "Histories",
        "Notes",
        limits,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut source = Cursor::new(bytes);
    let mut allowance = budget();
    let mut reader = VersionHistoryReader::open(
        &mut source,
        STORED_SCOPE,
        "Histories",
        "Notes",
        limits,
        &mut allowance,
        &|| false,
    )
    .unwrap();
    assert_eq!(reader.memo_column().name(), "Notes");
    assert_eq!(reader.parents().len(), 201);
    assert_eq!(
        reader
            .parents()
            .iter()
            .map(|parent| parent.child_count())
            .sum::<usize>(),
        20_000
    );
    for parent in STORED_PARENTS {
        let selected = reader.read_parent(parent).unwrap();
        observe::assert_history(&selected, parent);
        assert!(selected.row_reference() == full.links().parents()[parent].row_reference());
        assert_eq!(selected.key(), full.links().parents()[parent].key());
        assert_eq!(
            full.stored_versions(parent).unwrap().len(),
            selected.stored_versions().len()
        );
        for (stored, original) in selected
            .stored_versions()
            .zip(full.stored_versions(parent).unwrap())
        {
            let (stored, original) = (stored.unwrap(), original.unwrap());
            assert_eq!(stored.identity(), original.identity());
            assert_eq!(stored.value(), original.value());
            assert_eq!(
                stored.modified().map(|value| value.to_bits()),
                original.modified().map(|value| value.to_bits())
            );
        }
    }
    drop(reader);
    let mut source = Cursor::new(bytes);
    let mut allowance = budget();
    assert!(matches!(
        VersionHistoryReader::open(
            &mut source,
            STORED_SCOPE,
            "Histories",
            "Notes",
            TableSnapshotLimits::new(19_999, 256).unwrap(),
            &mut allowance,
            &|| false,
        ),
        Err(InspectError::StructureLimit)
    ));
}

#[test]
#[ignore = "requires registered ACE16 collection-scale originals"]
fn a_large_complete_census_consumes_the_original_budget_even_for_an_empty_next_parent() {
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
    let used = 1_000_000 - measured.remaining(Resource::Rows);
    assert!(used >= 2 * 68_000);
    let mut bounded = ResourceBudget::new(256 * 1024 * 1024, used, 100_000_000);
    let mut source = Cursor::new(bytes);
    let mut reader = MultivalueReader::open(
        &mut source,
        COLLECTION_SCOPE,
        "ManyParents",
        "Numbers",
        limits,
        &mut bounded,
        &|| false,
    )
    .unwrap();
    observe::assert_many_parent(&reader.read_parent(0).unwrap(), 0);
    let error = reader
        .read_parent(4)
        .err()
        .expect("a complete empty-parent refresh still consumes rows");
    assert!(matches!(error, InspectError::Limit(limit) if limit.resource == Resource::Rows));
    assert!(reader.is_failed());
    assert!(matches!(
        reader.read_parent(0),
        Err(InspectError::CollectionReaderFailed)
    ));
}
