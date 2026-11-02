//! Source-only literal Increment AFTERs; no generation service or planner.
use crate::{
    IndexCheckStatus, NativeImage, check_table, check_table_constraints,
    inspect_table_index_counter, inspect_table_properties, read_table_index,
    table::mutation::autonumber_counter,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(258);

struct Case {
    id: &'static str,
    before_name: &'static str,
    before_sha256: &'static str,
    family: &'static str,
    state: &'static str,
    table: &'static str,
    definition: usize,
    data: usize,
    root: usize,
    generated_position: usize,
    generated: i32,
    original_live: usize,
    original_slots: usize,
    last: i32,
    history: u32,
    record: &'static str,
    wire: &'static str,
}

const CASES: &[Case] = &[
    Case {
        id: "h5-increment-text-empty-inserted",
        before_name: "h5-increment-text-empty-before.accdb",
        before_sha256: "12946f4ebe11d2bae07ac9d9fa88633df99626b243097169814cae050a43542e",
        family: "text",
        state: "empty",
        table: "Items",
        definition: 7,
        data: 8,
        root: 9,
        generated_position: 0,
        generated: 1,
        original_live: 0,
        original_slots: 0,
        last: 1,
        history: 1,
        record: "030001000000110000006e006500770010000a00010007",
        wire: "7f8000000100000800",
    },
    Case {
        id: "h5-increment-text-high-inserted",
        before_name: "h5-increment-text-high-before.accdb",
        before_sha256: "52b642009b933cd05ca9ac128e3be71d9eb6b5c0aec1f12055881264f95afda6",
        family: "text",
        state: "high",
        table: "Items",
        definition: 7,
        data: 8,
        root: 9,
        generated_position: 0,
        generated: 138,
        original_live: 1,
        original_slots: 1,
        last: 138,
        history: 2,
        record: "03008a000000110000006e006500770010000a00010007",
        wire: "7f8000008a00000801",
    },
    Case {
        id: "h5-increment-text-companion-null-high-inserted",
        before_name: "h5-increment-text-companion-null-high-before.accdb",
        before_sha256: "4eba2cfc7010d814ff0680383aa883b53c8434739f962b1c5844564f6a167c7d",
        family: "text",
        state: "companion-null-high",
        table: "Items",
        definition: 7,
        data: 8,
        root: 9,
        generated_position: 0,
        generated: 138,
        original_live: 1,
        original_slots: 1,
        last: 138,
        history: 2,
        record: "03008a000000110000000a000a00010005",
        wire: "7f8000008a00000801",
    },
    Case {
        id: "h5-increment-binary-empty-inserted",
        before_name: "h5-increment-binary-empty-before.accdb",
        before_sha256: "b5eaf9b7ff845f4ae758e5372f5ec41158058b955860947afa2d2ea06f699337",
        family: "binary",
        state: "empty",
        table: "Items",
        definition: 7,
        data: 8,
        root: 9,
        generated_position: 0,
        generated: 1,
        original_live: 0,
        original_slots: 0,
        last: 1,
        history: 1,
        record: "03000100000011000000112233440e000a00010007",
        wire: "7f8000000100000800",
    },
    Case {
        id: "h5-increment-binary-high-inserted",
        before_name: "h5-increment-binary-high-before.accdb",
        before_sha256: "51383ed1ef58343fc0c0cd2040ca1e605d760edda5834cd22b03fd51f5918771",
        family: "binary",
        state: "high",
        table: "Items",
        definition: 7,
        data: 8,
        root: 9,
        generated_position: 0,
        generated: 138,
        original_live: 1,
        original_slots: 1,
        last: 138,
        history: 2,
        record: "03008a00000011000000112233440e000a00010007",
        wire: "7f8000008a00000801",
    },
    Case {
        id: "h5-increment-binary-companion-null-high-inserted",
        before_name: "h5-increment-binary-companion-null-high-before.accdb",
        before_sha256: "f97938689dbdeb607838db80472257e84ba0ee610e2c86d78bc04499a536fc56",
        family: "binary",
        state: "companion-null-high",
        table: "Items",
        definition: 7,
        data: 8,
        root: 9,
        generated_position: 0,
        generated: 138,
        original_live: 1,
        original_slots: 1,
        last: 138,
        history: 2,
        record: "03008a000000110000000a000a00010005",
        wire: "7f8000008a00000801",
    },
    Case {
        id: "h5-increment-fixed-empty-inserted",
        before_name: "h5-increment-fixed-empty-before.accdb",
        before_sha256: "3a3c81a274c1ee4cc075592626210bc4ccb7ed00473bc896567e22b551b07bcd",
        family: "fixed",
        state: "empty",
        table: "Items",
        definition: 7,
        data: 8,
        root: 9,
        generated_position: 0,
        generated: 1,
        original_live: 0,
        original_slots: 0,
        last: 1,
        history: 1,
        record: "030001000000f9ffffffffffffff05",
        wire: "7f8000000100000800",
    },
    Case {
        id: "h5-increment-fixed-high-inserted",
        before_name: "h5-increment-fixed-high-before.accdb",
        before_sha256: "7e782632784bc1e1c0a2ba40dbf1750bd2d546c14d638f5a407e122830fbd822",
        family: "fixed",
        state: "high",
        table: "Items",
        definition: 7,
        data: 8,
        root: 9,
        generated_position: 0,
        generated: 138,
        original_live: 1,
        original_slots: 1,
        last: 138,
        history: 2,
        record: "03008a000000f9ffffffffffffff05",
        wire: "7f8000008a00000801",
    },
    Case {
        id: "h5-increment-fixed-companion-null-high-inserted",
        before_name: "h5-increment-fixed-companion-null-high-before.accdb",
        before_sha256: "dbb7736298c4436df06ceb6f20579d61ebc930567d051efd32627dc6822cb43f",
        family: "fixed",
        state: "companion-null-high",
        table: "Items",
        definition: 7,
        data: 8,
        root: 9,
        generated_position: 0,
        generated: 138,
        original_live: 1,
        original_slots: 1,
        last: 138,
        history: 2,
        record: "03008a000000000000000000000001",
        wire: "7f8000008a00000801",
    },
    Case {
        id: "h6-increment-text-empty-inserted",
        before_name: "h6-increment-text-empty-before.accdb",
        before_sha256: "086f65f0bfad8f86543cb1acf3292249933c6067ae2d2b047b3b4fdc2118a6c9",
        family: "text",
        state: "empty",
        table: "Items",
        definition: 7,
        data: 8,
        root: 9,
        generated_position: 0,
        generated: 1,
        original_live: 0,
        original_slots: 0,
        last: 1,
        history: 1,
        record: "030001000000110000006e006500770010000a00010007",
        wire: "7f8000000100000800",
    },
    Case {
        id: "h6-increment-text-high-inserted",
        before_name: "h6-increment-text-high-before.accdb",
        before_sha256: "1f93544e1bbff7a433ad3b141c10d50106fa29454119f6d7fb358a4a4522940e",
        family: "text",
        state: "high",
        table: "Items",
        definition: 7,
        data: 8,
        root: 9,
        generated_position: 0,
        generated: 138,
        original_live: 1,
        original_slots: 1,
        last: 138,
        history: 2,
        record: "03008a000000110000006e006500770010000a00010007",
        wire: "7f8000008a00000801",
    },
    Case {
        id: "h6-increment-text-companion-null-high-inserted",
        before_name: "h6-increment-text-companion-null-high-before.accdb",
        before_sha256: "f95a3d83dc1320059916525e97f620065b3be16dfbb49961b7db17de3200c763",
        family: "text",
        state: "companion-null-high",
        table: "Items",
        definition: 7,
        data: 8,
        root: 9,
        generated_position: 0,
        generated: 138,
        original_live: 1,
        original_slots: 1,
        last: 138,
        history: 2,
        record: "03008a000000110000000a000a00010005",
        wire: "7f8000008a00000801",
    },
    Case {
        id: "h6-increment-binary-empty-inserted",
        before_name: "h6-increment-binary-empty-before.accdb",
        before_sha256: "fec61b9705ed92ace58e849fc9fcd9efa9f4ec182e94799faab99dca8784350c",
        family: "binary",
        state: "empty",
        table: "Items",
        definition: 7,
        data: 8,
        root: 9,
        generated_position: 0,
        generated: 1,
        original_live: 0,
        original_slots: 0,
        last: 1,
        history: 1,
        record: "03000100000011000000112233440e000a00010007",
        wire: "7f8000000100000800",
    },
    Case {
        id: "h6-increment-binary-high-inserted",
        before_name: "h6-increment-binary-high-before.accdb",
        before_sha256: "e81bf76e6b4e65dea20c7c13d8fff0d8c0b2c5e2b866eba3a3764bb0729a176f",
        family: "binary",
        state: "high",
        table: "Items",
        definition: 7,
        data: 8,
        root: 9,
        generated_position: 0,
        generated: 138,
        original_live: 1,
        original_slots: 1,
        last: 138,
        history: 2,
        record: "03008a00000011000000112233440e000a00010007",
        wire: "7f8000008a00000801",
    },
    Case {
        id: "h6-increment-binary-companion-null-high-inserted",
        before_name: "h6-increment-binary-companion-null-high-before.accdb",
        before_sha256: "e3b034d4015561ca9c195253691fd3687402da6501eeca8916db97cf0e946500",
        family: "binary",
        state: "companion-null-high",
        table: "Items",
        definition: 7,
        data: 8,
        root: 9,
        generated_position: 0,
        generated: 138,
        original_live: 1,
        original_slots: 1,
        last: 138,
        history: 2,
        record: "03008a000000110000000a000a00010005",
        wire: "7f8000008a00000801",
    },
    Case {
        id: "h6-increment-fixed-empty-inserted",
        before_name: "h6-increment-fixed-empty-before.accdb",
        before_sha256: "3fd3ca123c47a70a56e1111ef571227f8346c1896aa43d8a7b92d9e0a3ad456d",
        family: "fixed",
        state: "empty",
        table: "Items",
        definition: 7,
        data: 8,
        root: 9,
        generated_position: 0,
        generated: 1,
        original_live: 0,
        original_slots: 0,
        last: 1,
        history: 1,
        record: "030001000000f9ffffffffffffff05",
        wire: "7f8000000100000800",
    },
    Case {
        id: "h6-increment-fixed-high-inserted",
        before_name: "h6-increment-fixed-high-before.accdb",
        before_sha256: "a6e5065bb9f7c8eefc73bb6eff28278e2f053780b2b3b8a32b6860c6d49f774f",
        family: "fixed",
        state: "high",
        table: "Items",
        definition: 7,
        data: 8,
        root: 9,
        generated_position: 0,
        generated: 138,
        original_live: 1,
        original_slots: 1,
        last: 138,
        history: 2,
        record: "03008a000000f9ffffffffffffff05",
        wire: "7f8000008a00000801",
    },
    Case {
        id: "h6-increment-fixed-companion-null-high-inserted",
        before_name: "h6-increment-fixed-companion-null-high-before.accdb",
        before_sha256: "5083427c8bd09fb9e57978c83f512a457ebdf40efbc19e4e7dc1d71965f3bb39",
        family: "fixed",
        state: "companion-null-high",
        table: "Items",
        definition: 7,
        data: 8,
        root: 9,
        generated_position: 0,
        generated: 138,
        original_live: 1,
        original_slots: 1,
        last: 138,
        history: 2,
        record: "03008a000000000000000000000001",
        wire: "7f8000008a00000801",
    },
    Case {
        id: "h5-numbered-middle-inserted",
        before_name: "h5-default-original.accdb",
        before_sha256: "c6dc5d36f35335ea2d9cafbc5cf6500dc135ea41a4d3f60d9241ac9e55714d12",
        family: "genuine-middle",
        state: "deleted-high",
        table: "Numbered",
        definition: 94,
        data: 97,
        root: 96,
        generated_position: 1,
        generated: 4,
        original_live: 2,
        original_slots: 3,
        last: 4,
        history: 4,
        record: "030067000000040000001100000007",
        wire: "7f8000000400006103",
    },
    Case {
        id: "h6-numbered-middle-inserted",
        before_name: "h6-default-original.accdb",
        before_sha256: "c6aae0208a2dfbd0b77f8867f49700718880ee761c7881df6ec5789843a8b456",
        family: "genuine-middle",
        state: "deleted-high",
        table: "Numbered",
        definition: 111,
        data: 115,
        root: 113,
        generated_position: 1,
        generated: 4,
        original_live: 2,
        original_slots: 3,
        last: 4,
        history: 4,
        record: "030067000000040000001100000007",
        wire: "7f8000000400007303",
    },
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn hex(text: &str) -> Vec<u8> {
    assert!(text.len().is_multiple_of(2));
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn sha256(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn original_rows(case: &Case) -> Vec<Vec<Value>> {
    if case.family == "genuine-middle" {
        return vec![
            vec![Value::Int32(101), Value::Int32(1), Value::Int32(10)],
            vec![Value::Int32(102), Value::Int32(2), Value::Int32(20)],
        ];
    }
    if case.state == "empty" {
        return Vec::new();
    }
    let inactive = case.state == "companion-null-high";
    match case.family {
        "text" => vec![vec![
            Value::Int32(1),
            if inactive {
                Value::Null
            } else {
                Value::Text("old".into())
            },
            Value::Int32(10),
        ]],
        "binary" => vec![vec![
            Value::Int32(1),
            if inactive {
                Value::Null
            } else {
                Value::Binary(vec![0xc7, 0, 0xff])
            },
            Value::Int32(10),
        ]],
        "fixed" => vec![vec![
            Value::Int32(1),
            Value::Boolean(true),
            if inactive {
                Value::Null
            } else {
                Value::Int64(i64::MIN)
            },
        ]],
        _ => panic!("closed source family"),
    }
}

fn inserted_row(case: &Case) -> Vec<Value> {
    let inactive = case.state == "companion-null-high";
    match case.family {
        "text" => vec![
            Value::Int32(case.generated),
            if inactive {
                Value::Null
            } else {
                Value::Text("new".into())
            },
            Value::Int32(17),
        ],
        "binary" => vec![
            Value::Int32(case.generated),
            if inactive {
                Value::Null
            } else {
                Value::Binary(vec![0x11, 0x22, 0x33, 0x44])
            },
            Value::Int32(17),
        ],
        "fixed" => vec![
            Value::Int32(case.generated),
            Value::Boolean(false),
            if inactive {
                Value::Null
            } else {
                Value::Int64(-7)
            },
        ],
        "genuine-middle" => vec![Value::Int32(103), Value::Int32(4), Value::Int32(17)],
        _ => panic!("closed source family"),
    }
}

fn check(case: &Case, before: &[u8], after: &[u8]) {
    assert_eq!(before.len(), after.len());
    let original =
        NativeImage::capture(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false).unwrap();
    let source =
        super::super::capture_source(&original, case.table, &mut budget(), &|| false).unwrap();
    assert_eq!(source.table.rows(), original_rows(case));
    assert_eq!(source.records.len(), case.original_live);
    let image =
        NativeImage::capture(&mut Cursor::new(after), SCOPE, &mut budget(), &|| false).unwrap();
    let retained =
        super::super::capture_source(&image, case.table, &mut budget(), &|| false).unwrap();
    let mut rows = original_rows(case);
    rows.push(inserted_row(case));
    assert_eq!(retained.table.rows(), rows);
    assert_eq!(retained.records.len(), case.original_live + 1);
    assert_eq!(
        retained.table.schema().definition_page() as usize,
        case.definition
    );
    let counter = autonumber_counter::capture(
        &image,
        retained.table.schema(),
        case.generated_position,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(counter.last, case.last);
    assert_eq!(counter.offset, case.definition * 4096 + 20);
    for (old, kept) in source.records.iter().zip(&retained.records) {
        assert_eq!(old.reference, kept.reference);
        assert_eq!(old.offset, kept.offset);
        assert_eq!(old.data, kept.data);
    }
    let record = hex(case.record);
    let inserted = retained.records.last().unwrap();
    assert_eq!(
        inserted.reference,
        ((case.data as u32) << 8) | case.original_slots as u32
    );
    assert_eq!(inserted.data, record);
    let data = case.data * 4096;
    let start = inserted.offset - data;
    let end = start + record.len();
    assert_eq!(
        &after[data + 12..data + 14],
        &((case.original_slots + 1) as u16).to_le_bytes()
    );
    assert_eq!(
        &after[data + 14 + 2 * case.original_slots..data + 16 + 2 * case.original_slots],
        &(start as u16).to_le_bytes()
    );
    assert_eq!(
        &after[data + 2..data + 4],
        &((start - 14 - 2 * (case.original_slots + 1)) as u16).to_le_bytes()
    );

    let checked = check_table(
        &mut image.reader(),
        SCOPE,
        case.table,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), rows.len());
    assert_eq!(
        checked.indexes().len(),
        if case.family == "genuine-middle" {
            2
        } else {
            1
        }
    );
    for index in checked.indexes() {
        assert_eq!(index.status(), IndexCheckStatus::Checked);
        assert_eq!(index.index().root_page() as usize, case.root);
        assert_eq!(
            read_table_index(
                &mut image.reader(),
                SCOPE,
                case.table,
                index.index().name(),
                &mut budget(),
                &|| false
            )
            .unwrap()
            .rows(),
            rows
        );
        assert_eq!(
            inspect_table_index_counter(
                &mut image.reader(),
                SCOPE,
                case.table,
                index.index().name(),
                &mut budget(),
                &|| false
            )
            .unwrap()
            .value(),
            case.history
        );
    }
    assert!(
        check_table_constraints(
            &mut image.reader(),
            SCOPE,
            case.table,
            &mut budget(),
            &|| false
        )
        .unwrap()
        .relationships()
        .is_empty()
    );
    let properties = inspect_table_properties(
        &mut image.reader(),
        SCOPE,
        case.table,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(properties.raw_bytes().is_empty());
    assert!(properties.groups().is_empty());

    let root = case.root * 4096;
    let old_active = 9 * case.original_live;
    assert_eq!(
        &after[root + 480 + old_active..root + 489 + old_active],
        hex(case.wire)
    );
    assert_eq!(
        &after[root + 2..root + 4],
        &((3616 - old_active - 9) as u16).to_le_bytes()
    );
    let mut bitmap = vec![0; 453];
    for boundary in (9..=old_active + 9).step_by(9) {
        bitmap[boundary / 8] |= 1 << (boundary % 8);
    }
    assert_eq!(&after[root + 27..root + 480], bitmap);
    let td = case.definition * 4096;
    assert_eq!(
        &after[td + 16..td + 20],
        &((case.original_live + 1) as u32).to_le_bytes()
    );
    assert_eq!(&after[td + 20..td + 24], &case.last.to_le_bytes());
    assert_eq!(&after[td + 67..td + 71], &case.history.to_le_bytes());
    let mut ranges = vec![
        (data + start, data + end),
        (
            data + 14 + 2 * case.original_slots,
            data + 16 + 2 * case.original_slots,
        ),
        (data + 2, data + 4),
        (data + 12, data + 14),
        (td + 16, td + 24),
        (td + 67, td + 71),
        (root + 2, root + 4),
        (root + 27, root + 480),
        (root + 480 + old_active, root + 489 + old_active),
    ];
    ranges.sort_unstable();
    let mut cursor = 0;
    for (low, high) in ranges {
        assert!(cursor <= low && low < high && high <= after.len());
        assert_eq!(&before[cursor..low], &after[cursor..low]);
        cursor = high;
    }
    assert_eq!(&before[cursor..], &after[cursor..]);
    if case.family == "genuine-middle" {
        assert_eq!(&after[data + 18..data + 20], &[0xd3, 0xcf]);
        assert_eq!(
            &after[data + 4051..data + 4066],
            &hex("030067000000030000001e00000007")
        );
    }
}

#[test]
#[ignore = "requires independently modeled Increment AFTER20 and immutable qualified originals"]
fn companion_after_sources_match_presence_counter_physical_rows_and_full_protected_complement() {
    let output =
        PathBuf::from(std::env::var_os("TABLECRAFT_INCREMENT_COMPANION_AFTER_SOURCE_DIR").unwrap());
    let companions =
        PathBuf::from(std::env::var_os("TABLECRAFT_INCREMENT_COMPANION_SOURCE_DIR").unwrap());
    let genuine =
        PathBuf::from(std::env::var_os("TABLECRAFT_INCREMENT_COMPANION_ORIGINAL_DIR").unwrap());
    assert!(output.is_absolute() && companions.is_absolute() && genuine.is_absolute());
    let mut pins = BTreeMap::new();
    for line in fs::read_to_string(output.join("manifest.tsv"))
        .unwrap()
        .lines()
    {
        let (digest, name) = line.split_once('\t').unwrap();
        assert!(pins.insert(name.to_owned(), digest.to_owned()).is_none());
    }
    assert_eq!(pins.len(), 20);
    let mut inputs = Vec::new();
    for case in CASES {
        let directory = if case.family == "genuine-middle" {
            &genuine
        } else {
            &companions
        };
        let before = fs::read(directory.join(case.before_name)).unwrap();
        assert_eq!(sha256(&before), case.before_sha256);
        let name = format!("{}.accdb", case.id);
        let after = fs::read(output.join(&name)).unwrap();
        assert_eq!(sha256(&after), pins.remove(&name).unwrap());
        inputs.push((case, before, after));
    }
    assert!(pins.is_empty());
    // All original/AFTER file hashes are verified before any native capture.
    for (case, before, after) in inputs {
        check(case, &before, &after);
    }
}
