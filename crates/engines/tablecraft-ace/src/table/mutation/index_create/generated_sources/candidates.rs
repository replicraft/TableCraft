//! Finite ignored consumer of prefrozen genuine-original expectations.
use super::{
    REQUESTS, SCOPE, SOURCES, SecondaryIndexCreate, bounded_read, budget, existing_index_pages,
    source,
};
use crate::{
    IndexCheckStatus, IndexProjectionSelection, IndexReadRequest, NativeImage, check_table,
    inspect_table_index_counter, read_table_index_with_references,
    table::mutation::append::owners::MultipleOwners,
};
use sha2::{Digest, Sha256};
use std::{
    fmt::Write as FmtWrite,
    fs::{self, File, OpenOptions},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
};
use tablecraft_core::values::Value;

#[path = "candidates/expected.rs"]
mod expected;

const MAX_PIN_BYTES: usize = 8 * 1024 * 1024;
const OUTPUT: &str = ".cache/ace16-index-create-genuine-candidates-first-20261030";
const PINS: &str = include_str!("candidates/pins.tsv");

fn lower_hex(bytes: &[u8]) -> String {
    let mut result = String::new();
    for byte in bytes {
        write!(result, "{byte:02x}").unwrap();
    }
    result
}

fn fixture_root() -> PathBuf {
    let selected = std::env::var_os("TABLECRAFT_FIXTURE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.."));
    let root = selected
        .canonicalize()
        .expect("engineering fixture root must exist");
    assert!(
        root.is_dir(),
        "engineering fixture root must be a directory"
    );
    root
}

fn pinned_read(root: &Path, path: &str, length: usize, digest: &str) -> Vec<u8> {
    assert!(length <= MAX_PIN_BYTES);
    let file = File::open(root.join(path)).expect("frozen input must exist");
    assert_eq!(file.metadata().unwrap().len(), length as u64);
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length + 1).unwrap();
    file.take(length as u64 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(bytes.len(), length);
    assert_eq!(lower_hex(&Sha256::digest(&bytes)), digest);
    bytes
}

fn verify_inputs(root: &Path) {
    for line in PINS.lines() {
        let fields = line.split('\t').collect::<Vec<_>>();
        assert_eq!(fields.len(), 3);
        let length = fields[1].parse().unwrap();
        let _verified = pinned_read(root, fields[0], length, fields[2]);
    }
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn exact_image(actual: &[u8], frozen: &[u8], case: &str) {
    assert_eq!(actual.len(), frozen.len(), "case={case}");
    assert!(
        actual == frozen,
        "case={case}; first differing offset={:?}",
        actual
            .iter()
            .zip(frozen)
            .position(|(left, right)| left != right),
    );
}

fn readback(bytes: &[u8], binding: &expected::Expected) {
    let image = capture(bytes);
    let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), binding.rows);
    assert_eq!(checked.schema().definition_page(), binding.definition);
    assert_eq!(checked.indexes().len(), 5);
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| { index.status() == IndexCheckStatus::Checked })
    );
    let selected = checked
        .indexes()
        .iter()
        .find(|index| index.index().name() == binding.name)
        .unwrap()
        .index();
    assert_eq!(selected.number(), 4);
    assert_eq!(selected.data_number(), 2);
    assert_eq!(selected.flags(), 128);
    assert_eq!(selected.native_type(), 0);
    assert_eq!(selected.root_page(), binding.root);
    assert_eq!(selected.usage_map(), binding.container << 8);
    assert_eq!(selected.columns().len(), 1);
    assert_eq!(selected.columns()[0].number(), 1);
    assert_eq!(selected.columns()[0].descending(), binding.descending);

    let counter = inspect_table_index_counter(
        &mut image.reader(),
        SCOPE,
        "Items",
        binding.name,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(counter.value(), binding.counter);
    assert_eq!(counter.definition_page(), binding.definition);
    assert_eq!(counter.physical_index(), 2);

    let projected = read_table_index_with_references(
        &mut image.reader(),
        SCOPE,
        "Items",
        binding.name,
        IndexReadRequest {
            columns: Some(&["Id", "Score", "Guard"]),
            selection: IndexProjectionSelection::All,
            maximum_rows: u32::try_from(binding.rows.max(1)).unwrap(),
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(projected.snapshot().rows().len(), binding.rows);
    assert_eq!(projected.row_references().len(), binding.rows);
    let mut order = Sha256::new();
    for (row, reference) in projected
        .snapshot()
        .rows()
        .iter()
        .zip(projected.row_references())
    {
        let packed = reference.page_number() << 8 | u32::from(reference.slot_number());
        order.update(packed.to_be_bytes());
        let [Value::Int32(identity), score, Value::Int32(guard)] = row.as_slice() else {
            panic!("case={}; wrong projected carriers", binding.id);
        };
        let position = *identity - 1;
        assert!(position >= 0 && (position as usize) < binding.rows);
        assert_eq!(*guard, position % 7);
        let expected_score = match binding.population {
            "distinct" => Some(position - 1),
            "nullable" => [None, Some(-7), Some(-7), Some(0), None, Some(7)][position as usize],
            "pressure" => (position % 10 != 0).then_some(position - 300),
            _ => panic!("nonempty unexpected population"),
        };
        match expected_score {
            Some(value) => assert_eq!(score, &Value::Int32(value)),
            None => assert_eq!(score, &Value::Null),
        }
    }
    assert_eq!(lower_hex(&order.finalize()), binding.order_sha256);
}

#[test]
#[ignore = "requires immutable private originals and sixteen prefrozen complete images"]
fn genuine_secondary_index_candidates_match_all_prefrozen_images() {
    let root = fixture_root();
    verify_inputs(&root);
    assert_eq!(expected::EXPECTED.len(), REQUESTS.len());
    let originals = SOURCES
        .iter()
        .map(|binding| bounded_read(&root, binding))
        .collect::<Vec<_>>();
    let frozen = expected::EXPECTED
        .iter()
        .map(|binding| pinned_read(&root, binding.path, binding.bytes, binding.sha256))
        .collect::<Vec<_>>();

    // Every source/request/expectation is qualified before writer call one.
    for ((intent, binding), expected) in REQUESTS.iter().zip(&expected::EXPECTED).zip(&frozen) {
        assert_eq!(intent.id, binding.id);
        assert_eq!(intent.source, binding.source_id);
        assert_eq!(intent.name, binding.name);
        assert_eq!(intent.descending, binding.descending);
        let position = SOURCES
            .iter()
            .position(|source| source.id == intent.source)
            .unwrap();
        let image = capture(&originals[position]);
        let request = SecondaryIndexCreate::new(intent.name, intent.column, intent.descending);
        let original =
            source::qualify(&image, intent.table, &request, &mut budget(), &|| false).unwrap();
        assert_eq!(original.checked.rows(), binding.rows);
        let pages = existing_index_pages(&image, &original.checked).unwrap();
        drop(
            MultipleOwners::capture(&image, &[&original.checked], &pages, &mut budget(), &|| {
                false
            })
            .unwrap(),
        );
        readback(expected, binding);
    }
    verify_inputs(&root);
    let output = root.join(OUTPUT);
    assert!(!output.exists());
    fs::create_dir(&output).unwrap();

    for ((intent, binding), expected) in REQUESTS.iter().zip(&expected::EXPECTED).zip(&frozen) {
        verify_inputs(&root);
        let position = SOURCES
            .iter()
            .position(|source| source.id == intent.source)
            .unwrap();
        let request = SecondaryIndexCreate::new(intent.name, intent.column, intent.descending);
        let candidate = capture(&originals[position])
            .preview_ace16_create_secondary_index(intent.table, &request, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(candidate.before_bytes(), originals[position]);
        assert_eq!(candidate.index_name(), binding.name);
        assert_eq!(candidate.logical_index_number(), 4);
        assert_eq!(candidate.physical_index_number(), 2);
        assert_eq!(candidate.indexed_rows(), binding.rows);
        exact_image(candidate.candidate_bytes(), expected, intent.id);
        readback(candidate.candidate_bytes(), binding);

        let path = output.join(format!("{}.accdb", binding.id));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        file.write_all(candidate.candidate_bytes()).unwrap();
        file.sync_all().unwrap();
        println!(
            "MATCH\t{}\tbytes={}\trows={}\tcounter={}",
            binding.id, binding.bytes, binding.rows, binding.counter
        );
    }
    verify_inputs(&root);
    for binding in &SOURCES {
        let _unchanged = bounded_read(&root, binding);
    }
    println!("COMPLETE\twriter_calls=16\twhole_images=16\toriginals_unchanged=8");
}
