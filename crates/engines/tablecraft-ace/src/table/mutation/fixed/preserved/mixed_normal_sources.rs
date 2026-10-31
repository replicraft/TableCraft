//! Read-only reuse of the actual rich owner; no fixture graph is copied.
use super::{rich_before_change_sources::model as rich, source as preserved_source};
use crate::{
    BeforeEventGuardError, BeforeEventGuardSelector, BeforeEventIntents, BeforeGuardUnsupported,
    EditError, InspectError, NativeImage, PayloadPresence, TableExtraCoverage, WriteBlocker,
    inspect_table_schema, read_table_projection,
    table::mutation::guards::fields::batch::SelectedPrograms,
};
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    io::{Cursor, Write},
    path::Path,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(101);

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000)
}

fn source(code: u8, counter: bool) -> (rich::Case, Option<Vec<u8>>, i32) {
    let original = rich::source(
        code,
        if counter {
            rich::Recipe::CounterRead
        } else {
            rich::Recipe::Int32Sequential
        },
    );
    let final_value: i32 = if counter { 1 } else { 1018 };
    // Original user Large_Number1000→17. Exact existing XML either sets it to
    // ID1, or applies17 + ID1 + Old1000. No evaluator or writer constructs this.
    let expected = (code == 6).then(|| {
        let mut after = original.before.clone();
        let word = 12 * 4096 + 14;
        let start = 12 * 4096
            + usize::from(u16::from_le_bytes(
                after[word..word + 2].try_into().unwrap(),
            ));
        assert_eq!(&after[start..start + 2], &16_u16.to_le_bytes());
        let patch = start + 2 + 8;
        assert_eq!(&after[patch..patch + 8], &1000_i64.to_le_bytes());
        after[patch..patch + 4].copy_from_slice(&final_value.to_le_bytes());
        assert_eq!(&after[..patch], &original.before[..patch]);
        assert_eq!(&after[patch + 4..], &original.before[patch + 4..]);
        after
    });
    (original, expected, final_value)
}

fn qualify(case: &rich::Case, code: u8, bytes: &[u8], target: i32) {
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap();
    let schema = inspect_table_schema(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(schema.columns().len(), 16);
    assert_eq!(
        (
            schema.columns()[0].native_type(),
            schema.columns()[0].flags(),
            schema.columns()[0].storage_length()
        ),
        (4, 7, 4)
    );
    assert_eq!(
        (
            schema.columns()[4].name(),
            schema.columns()[4].native_type(),
            schema.columns()[4].flags(),
            schema.columns()[4].extended_flags(),
            schema.columns()[4].storage_length()
        ),
        ("Large_Number", 4, 3, 0, 4)
    );
    let projection = read_table_projection(
        &mut image.reader(),
        SCOPE,
        "Items",
        &["ID", "Number", "Large_Number"],
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(projection.rows().len(), 3);
    assert_eq!(projection.rows()[0][0], Value::Int32(1));
    assert_eq!(projection.rows()[0][2], Value::Int32(target));
    let coverage = image
        .inspect_table_macro_coverage(schema.object_id(), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(coverage.body().presence(), PayloadPresence::Absent);
    assert_eq!(coverage.module().presence(), PayloadPresence::Absent);
    assert!(coverage.children().is_empty());
    assert_eq!(
        coverage.table_extra().coverage(),
        TableExtraCoverage::WideMr2
    );
    assert_eq!(coverage.table_extra().raw(), Some(case.extra.as_slice()));
    let selectors = case
        .selection
        .iter()
        .map(|index| BeforeEventGuardSelector::new(3, &case.properties[*index].0))
        .collect::<Vec<_>>();
    // This existing selected read seal only permits the existing preservation
    // Source to inspect the exact Extra. It is never normal-save permission.
    let selected =
        SelectedPrograms::capture(&image, &schema, &selectors, &mut budget(), &|| false).unwrap();
    let admitted =
        preserved_source::capture_before_change(&image, "Items", &selected, &mut budget(), &|| {
            false
        });
    if code == 5 {
        assert!(matches!(
            admitted,
            Err(EditError::Unsupported(WriteBlocker::Column))
        ));
        for number in [7, 8] {
            assert_eq!(schema.columns()[number].native_type(), 20);
            assert!(schema.columns()[number].scalar_type().is_none());
        }
    } else {
        let admitted = admitted.unwrap();
        assert_eq!(admitted.records.len(), 3);
        assert!(!admitted.dependencies.contains(&4));
        assert!(
            admitted
                .indexes
                .indexes()
                .iter()
                .all(|index| index.columns().iter().all(|key| key.number() != 4))
        );
    }
    let old = image
        .prepare_native_before_change_fields(
            schema.object_id(),
            BeforeEventIntents::FixedUpdate {
                column: "Large_Number",
                updates: &[],
            },
            &mut budget(),
            &|| false,
        )
        .err()
        .expect("old complete logical row profile cannot manufacture rich cells");
    assert!(matches!(
        old,
        BeforeEventGuardError::Inspect(InspectError::Unsupported)
            | BeforeEventGuardError::Unsupported(BeforeGuardUnsupported::SourceProfile)
            | BeforeEventGuardError::Intent(EditError::Unsupported(WriteBlocker::Column))
    ));
    assert_eq!(image.as_bytes(), bytes);
}

#[test]
fn qualify_actual_rich_originals_and_literal_i32_images_before_normal_projection() {
    let mut admitted = 0;
    let mut refused = 0;
    for code in [5, 6] {
        for counter in [false, true] {
            let (case, expected, value) = source(code, counter);
            qualify(&case, code, &case.before, 1000);
            if code == 5 {
                assert!(expected.is_none());
                refused += 1;
            } else {
                admitted += 1;
                qualify(&case, code, expected.as_ref().unwrap(), value);
                admitted += 1;
            }
        }
    }
    assert_eq!((admitted, refused), (4, 2));
}

fn create(path: &Path, bytes: &[u8]) {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}

#[test]
#[ignore = "explicit fresh absolute directory; actual rich owner remains immutable"]
fn export_actual_rich_mixed_sources_and_literal_expectations_create_new() {
    let directory =
        env::var_os("TABLECRAFT_MIXED_RICH_SOURCE_DIR").expect("explicit output directory");
    let directory = Path::new(&directory);
    assert!(directory.is_absolute());
    assert!(!directory.exists());
    let mut files = Vec::new();
    for code in [5, 6] {
        for counter in [false, true] {
            let (case, expected, _) = source(code, counter);
            let id = if counter {
                "rich-counter-read"
            } else {
                "rich-sequential"
            };
            files.push((format!("h{code}-{id}-original.accdb"), case.before));
            if let Some(expected) = expected {
                files.push((format!("h{code}-{id}-expected.accdb"), expected));
            }
        }
    }
    assert_eq!(files.len(), 6);
    fs::create_dir(directory).unwrap();
    let mut inventory = Vec::new();
    for (name, bytes) in files {
        create(&directory.join(&name), &bytes);
        let hash = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let pin = format!(
            "{{\"path\":\"{name}\",\"bytes\":{},\"sha256\":\"{hash}\"}}",
            bytes.len()
        );
        println!("{pin}");
        inventory.push(pin);
    }
    create(
        &directory.join("inventory.json"),
        format!("{{\"files\":[{}]}}\n", inventory.join(",")).as_bytes(),
    );
    create(
        &directory.join("requests.json"),
        concat!(
            "{\"cases\":[",
            "{\"id\":\"rich-sequential\",\"column\":\"Large_Number\",\"ordinal\":0,\"expected\":1000,\"replacement\":17,\"final\":1018},",
            "{\"id\":\"rich-counter-read\",\"column\":\"Large_Number\",\"ordinal\":0,\"expected\":1000,\"replacement\":17,\"final\":1}",
            "],\"header5\":\"preserved-source-column-refusal\",",
            "\"header6\":\"candidate\"}\n",
        )
        .as_bytes(),
    );
}

#[test]
fn rich_mixed_sources_keep_first_export_pins() {
    let pins = [
        (
            5,
            false,
            false,
            "4a387a3eb723db54a0442a5dbeaad136f608d5619ddd6c671511ce2e63f7d092",
        ),
        (
            5,
            true,
            false,
            "9b14b51b014790b725311ac5cc68460e4f7a5a96df68f26203f6a7ebec47e4c7",
        ),
        (
            6,
            false,
            false,
            "78fa391045b363f430534452304b0b9810309aa1506d3d791a4cc0b99384e633",
        ),
        (
            6,
            false,
            true,
            "d92f70c042418d9a26d306b86da19a3f7dffa57d55643c9d486262ef55126202",
        ),
        (
            6,
            true,
            false,
            "16254087b3aa02bb429060683278010996914f270c61101b44c02e9a6bb8de62",
        ),
        (
            6,
            true,
            true,
            "341addbc391b463cbddd0f016b85769f52442d410f1f44570e56abc5bc0c8f28",
        ),
    ];

    for (header, counter, expected, pin) in pins {
        let (original, after, _) = source(header, counter);
        let raw = if expected {
            after.as_deref().expect("qualified header-6 expectation")
        } else {
            &original.before
        };
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, pin);
    }
}
