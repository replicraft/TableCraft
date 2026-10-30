//! Finite private source and candidate qualification of immutable originals.
mod candidates;
use super::{SecondaryIndexCreate, source};
use crate::{
    EditError, NativeImage, TableCheck, check_table,
    format::{allocation, reader::Reader},
    table::mutation::append::owners::MultipleOwners,
};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Cursor, Read},
    path::Path,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const MAX_INPUT_BYTES: u64 = 8 * 1024 * 1024;
const SCOPE: CatalogScope = CatalogScope::new(1);

struct Source {
    id: &'static str,
    path: &'static str,
    bytes: usize,
    digest: [u8; 32],
    rows: usize,
}

struct Request {
    id: &'static str,
    source: &'static str,
    table: &'static str,
    column: &'static str,
    name: &'static str,
    descending: bool,
}
const SOURCES: [Source; 8] = [
    Source {
        id: "h5-empty",
        path: ".cache/ace16-secondary-index-public-reference-20261030/generated-sources/h5-empty.accdb",
        bytes: 397312,
        digest: [
            0x62, 0x0d, 0x35, 0x14, 0x27, 0xf9, 0x3f, 0x8f, 0x34, 0xdc, 0x14, 0x0a, 0xf7, 0x37,
            0xfb, 0x44, 0xdb, 0xb1, 0x94, 0xa3, 0x11, 0x4d, 0xe5, 0xbb, 0xe8, 0x2f, 0x74, 0xad,
            0xfa, 0x4c, 0x59, 0x21,
        ],
        rows: 0,
    },
    Source {
        id: "h5-distinct",
        path: ".cache/ace16-secondary-index-public-reference-20261030/generated-sources/h5-distinct.accdb",
        bytes: 405504,
        digest: [
            0x72, 0xe7, 0xba, 0x54, 0x80, 0xd9, 0x5d, 0x0b, 0xe0, 0xd4, 0xea, 0x6f, 0x2c, 0x75,
            0xb0, 0x15, 0x06, 0x29, 0x70, 0x59, 0x9d, 0xa3, 0xfd, 0xe6, 0x20, 0xcc, 0x9f, 0x13,
            0xc6, 0x0b, 0x02, 0x8d,
        ],
        rows: 3,
    },
    Source {
        id: "h5-nullable",
        path: ".cache/ace16-secondary-index-public-reference-20261030/generated-sources/h5-nullable.accdb",
        bytes: 405504,
        digest: [
            0xe4, 0x4c, 0x5b, 0x6c, 0xa3, 0x74, 0x05, 0xf5, 0xf9, 0xbb, 0x9e, 0xd4, 0xff, 0x49,
            0xdc, 0x94, 0x0d, 0x64, 0xd3, 0x82, 0x95, 0x28, 0xe8, 0xb6, 0xc0, 0x03, 0x09, 0x0a,
            0xcc, 0x28, 0x23, 0xd1,
        ],
        rows: 6,
    },
    Source {
        id: "h5-pressure",
        path: ".cache/ace16-secondary-index-public-reference-20261030/generated-sources/h5-pressure.accdb",
        bytes: 413696,
        digest: [
            0x30, 0x61, 0x7d, 0x80, 0xe3, 0x42, 0xed, 0x3c, 0x86, 0xf6, 0x7b, 0xbc, 0x9b, 0xa4,
            0xd2, 0xd4, 0xe6, 0x04, 0x72, 0x63, 0x2f, 0xbc, 0xce, 0xca, 0x30, 0x36, 0xa5, 0xaf,
            0x76, 0x88, 0x96, 0x48,
        ],
        rows: 600,
    },
    Source {
        id: "h6-empty",
        path: ".cache/ace16-secondary-index-public-reference-20261030/generated-sources/h6-empty.accdb",
        bytes: 466944,
        digest: [
            0xe5, 0xc1, 0x65, 0x5e, 0x99, 0x9f, 0x2a, 0xbb, 0x9d, 0xfd, 0xa2, 0x86, 0x1b, 0xee,
            0xf8, 0x35, 0x88, 0xa1, 0x76, 0x05, 0x08, 0x08, 0xa4, 0x2b, 0xdf, 0xf6, 0x10, 0xb5,
            0x36, 0x86, 0xc2, 0x8a,
        ],
        rows: 0,
    },
    Source {
        id: "h6-distinct",
        path: ".cache/ace16-secondary-index-public-reference-20261030/generated-sources/h6-distinct.accdb",
        bytes: 475136,
        digest: [
            0x5c, 0xfb, 0xf6, 0xd4, 0xb6, 0x22, 0x4b, 0x98, 0x9f, 0x00, 0xf0, 0xab, 0xd6, 0x3e,
            0xc3, 0x62, 0x84, 0x6f, 0xf6, 0x02, 0x40, 0xa9, 0x1b, 0x05, 0x8e, 0x14, 0x42, 0xc3,
            0x01, 0x03, 0x05, 0xc5,
        ],
        rows: 3,
    },
    Source {
        id: "h6-nullable",
        path: ".cache/ace16-secondary-index-public-reference-20261030/generated-sources/h6-nullable.accdb",
        bytes: 475136,
        digest: [
            0xdf, 0x1a, 0x24, 0x5a, 0x66, 0xab, 0x4d, 0x14, 0xda, 0x9a, 0x6b, 0xf3, 0xdf, 0xd6,
            0xf2, 0xdf, 0x88, 0x1c, 0x21, 0xb2, 0xb5, 0xf2, 0x89, 0x54, 0xf5, 0xb1, 0x10, 0x3c,
            0x4c, 0x5d, 0x1a, 0xae,
        ],
        rows: 6,
    },
    Source {
        id: "h6-pressure",
        path: ".cache/ace16-secondary-index-public-reference-20261030/generated-sources/h6-pressure.accdb",
        bytes: 483328,
        digest: [
            0xde, 0x91, 0x07, 0xc0, 0x3a, 0x1f, 0x87, 0xeb, 0x4c, 0xa0, 0x91, 0xe9, 0xc8, 0x1a,
            0x90, 0x97, 0xb8, 0x2a, 0x50, 0x52, 0xc6, 0x45, 0x1c, 0x0b, 0x00, 0x9c, 0x2f, 0x68,
            0xc9, 0x8c, 0x1a, 0x51,
        ],
        rows: 600,
    },
];

const REQUESTS: [Request; 16] = [
    Request {
        id: "h5-empty-asc",
        source: "h5-empty",
        table: "Items",
        column: "Score",
        name: "ScoreLookupAsc",
        descending: false,
    },
    Request {
        id: "h5-empty-desc",
        source: "h5-empty",
        table: "Items",
        column: "Score",
        name: "ScoreLookupDesc",
        descending: true,
    },
    Request {
        id: "h5-distinct-asc",
        source: "h5-distinct",
        table: "Items",
        column: "Score",
        name: "ScoreLookupAsc",
        descending: false,
    },
    Request {
        id: "h5-distinct-desc",
        source: "h5-distinct",
        table: "Items",
        column: "Score",
        name: "ScoreLookupDesc",
        descending: true,
    },
    Request {
        id: "h5-nullable-asc",
        source: "h5-nullable",
        table: "Items",
        column: "Score",
        name: "ScoreLookupAsc",
        descending: false,
    },
    Request {
        id: "h5-nullable-desc",
        source: "h5-nullable",
        table: "Items",
        column: "Score",
        name: "ScoreLookupDesc",
        descending: true,
    },
    Request {
        id: "h5-pressure-asc",
        source: "h5-pressure",
        table: "Items",
        column: "Score",
        name: "ScoreLookupAsc",
        descending: false,
    },
    Request {
        id: "h5-pressure-desc",
        source: "h5-pressure",
        table: "Items",
        column: "Score",
        name: "ScoreLookupDesc",
        descending: true,
    },
    Request {
        id: "h6-empty-asc",
        source: "h6-empty",
        table: "Items",
        column: "Score",
        name: "ScoreLookupAsc",
        descending: false,
    },
    Request {
        id: "h6-empty-desc",
        source: "h6-empty",
        table: "Items",
        column: "Score",
        name: "ScoreLookupDesc",
        descending: true,
    },
    Request {
        id: "h6-distinct-asc",
        source: "h6-distinct",
        table: "Items",
        column: "Score",
        name: "ScoreLookupAsc",
        descending: false,
    },
    Request {
        id: "h6-distinct-desc",
        source: "h6-distinct",
        table: "Items",
        column: "Score",
        name: "ScoreLookupDesc",
        descending: true,
    },
    Request {
        id: "h6-nullable-asc",
        source: "h6-nullable",
        table: "Items",
        column: "Score",
        name: "ScoreLookupAsc",
        descending: false,
    },
    Request {
        id: "h6-nullable-desc",
        source: "h6-nullable",
        table: "Items",
        column: "Score",
        name: "ScoreLookupDesc",
        descending: true,
    },
    Request {
        id: "h6-pressure-asc",
        source: "h6-pressure",
        table: "Items",
        column: "Score",
        name: "ScoreLookupAsc",
        descending: false,
    },
    Request {
        id: "h6-pressure-desc",
        source: "h6-pressure",
        table: "Items",
        column: "Score",
        name: "ScoreLookupDesc",
        descending: true,
    },
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn bounded_read(root: &Path, source: &Source) -> Vec<u8> {
    assert!(source.bytes as u64 <= MAX_INPUT_BYTES);
    let file = File::open(root.join(source.path)).expect("pinned original source must exist");
    assert_eq!(file.metadata().unwrap().len(), source.bytes as u64);
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(source.bytes + 1).unwrap();
    file.take(source.bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(bytes.len(), source.bytes);
    let actual: [u8; 32] = Sha256::digest(&bytes).into();
    assert_eq!(
        actual, source.digest,
        "original digest mismatch for {}",
        source.id
    );
    bytes
}

fn existing_index_pages(image: &NativeImage, checked: &TableCheck) -> Result<Vec<u32>, EditError> {
    let mut work = budget();
    let mut input = image.reader();
    let mut reader = Reader {
        source: &mut input,
        header: image.catalog().header(),
        budget: &mut work,
        cancelled: &|| false,
        reads: 0,
    };
    let mut pages = Vec::new();
    let mut physical = Vec::new();
    for checked_index in checked.indexes() {
        let index = checked_index.index();
        reader.work(physical.len())?;
        if physical.contains(&index.data_number()) {
            continue;
        }
        reader
            .budget
            .charge_allocation(1, size_of::<u32>() as u64)
            .map_err(crate::InspectError::from)?;
        physical
            .try_reserve(1)
            .map_err(|_| crate::InspectError::Allocation)?;
        physical.push(index.data_number());
        for page in allocation::pages(&mut reader, index.usage_map())? {
            reader.work(pages.len())?;
            if !pages.contains(&page) {
                reader
                    .budget
                    .charge_allocation(1, size_of::<u32>() as u64)
                    .map_err(crate::InspectError::from)?;
                pages
                    .try_reserve(1)
                    .map_err(|_| crate::InspectError::Allocation)?;
                pages.push(page);
            }
        }
    }
    Ok(pages)
}

#[test]
#[ignore = "requires eight immutable private originals; source-only admission diagnostic"]
fn diagnose_generated_secondary_index_original_source_admission() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    // All eight original byte/hash bindings and all sixteen immutable request
    // bindings are fixed BEFORE the first engine capture/qualification call.
    let originals = SOURCES
        .iter()
        .map(|source| bounded_read(&root, source))
        .collect::<Vec<_>>();
    assert_eq!(REQUESTS.len(), 16);
    for request in &REQUESTS {
        assert!(SOURCES.iter().any(|source| source.id == request.source));
        assert_eq!(request.table, "Items");
        assert_eq!(request.column, "Score");
        assert_eq!(
            request.name,
            if request.descending {
                "ScoreLookupDesc"
            } else {
                "ScoreLookupAsc"
            }
        );
    }

    let mut requests = 0;
    let mut qualified = 0;
    let mut source_calls = 0;
    let mut ownership_calls = 0;
    for (binding, bytes) in SOURCES.iter().zip(&originals) {
        let image =
            match NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false) {
                Ok(image) => {
                    assert_eq!(image.as_bytes(), bytes);
                    println!("CAPTURE\t{}\tretained\tbytes={}", binding.id, bytes.len());
                    Some(image)
                }
                Err(error) => {
                    println!("CAPTURE\t{}\trefused\t{error:?}", binding.id);
                    None
                }
            };
        for intent in REQUESTS.iter().filter(|intent| intent.source == binding.id) {
            requests += 1;
            let Some(image) = image.as_ref() else {
                println!("SOURCE\t{}\tskipped_after_capture_refusal", intent.id);
                continue;
            };
            let request = SecondaryIndexCreate::new(intent.name, intent.column, intent.descending);
            source_calls += 1;
            match source::qualify(image, intent.table, &request, &mut budget(), &|| false) {
                Ok(original) => {
                    assert_eq!(original.checked.rows(), binding.rows);
                    println!(
                        "SOURCE\t{}\tsource_qualified\trows={}\tcolumn={}\tdescending={}",
                        intent.id,
                        original.checked.rows(),
                        original.column,
                        original.descending
                    );
                    qualified += 1;
                }
                Err(error) => println!("SOURCE\t{}\trefused\t{error:?}", intent.id),
            }
        }
        let Some(image) = image.as_ref() else {
            continue;
        };
        // These original readers are intentionally independent of source::qualify:
        // record their own typed blockers rather than forcing prior eligibility.
        match check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
            false
        }) {
            Ok(checked) => {
                assert_eq!(checked.rows(), binding.rows);
                println!(
                    "ORIGINAL_READ\t{}\tread\trows={}\tlogical_indexes={}",
                    binding.id,
                    checked.rows(),
                    checked.indexes().len()
                );
                match existing_index_pages(image, &checked) {
                    Ok(pages) => {
                        ownership_calls += 1;
                        // Actual immutable old-owner census only. No FreshIndexOwner,
                        // State, reservation, prospective GLOBAL cell or writer.
                        match MultipleOwners::capture(
                            image,
                            &[&checked],
                            &pages,
                            &mut budget(),
                            &|| false,
                        ) {
                            Ok(original_owners) => {
                                drop(original_owners);
                                println!(
                                    "ORIGINAL_OWNERS\t{}\tcaptured\tindex_owned_pages={}",
                                    binding.id,
                                    pages.len()
                                );
                            }
                            Err(error) => {
                                println!("ORIGINAL_OWNERS\t{}\trefused\t{error:?}", binding.id)
                            }
                        }
                    }
                    Err(error) => println!("ORIGINAL_MAPS\t{}\trefused\t{error:?}", binding.id),
                }
            }
            Err(error) => println!("ORIGINAL_READ\t{}\trefused\t{error:?}", binding.id),
        }
    }
    // This test reports eligibility outcomes, not a requirement that any genuine
    // source pass the current engineering profile. Only input integrity is fixed.
    assert_eq!(requests, 16);
    for binding in &SOURCES {
        let _unchanged = bounded_read(&root, binding);
    }
    println!(
        "COMPLETE\toriginals=8\trequests={requests}\tsource_calls={source_calls}\tsource_qualified={qualified}\townership_calls={ownership_calls}\twriters=0\toriginals_unchanged=8"
    );
}
