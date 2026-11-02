//! Exclusive source export and literal readonly-observer expectations.
use super::super::export::{cell, expected as base_expected};
use super::{model, source};
use sha2::{Digest, Sha256};
use std::{env, fs, io::Write, path::Path};
use tablecraft_core::values::Value;
use tablecraft_testkit::fixtures::access::canonical_ordinary_content::{
    Source as BaseSource, Table,
};

const PAGE: usize = 4096;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
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

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn declarations(original: &model::Source) -> String {
    let name = hex(original.table.as_bytes());
    let mut output = String::new();
    if let Some(last) = original.last_counter {
        assert_eq!(u32_at(&original.bytes, 7 * PAGE + 20) as i32, last);
        output.push_str(&format!("COUNTER\t{name}\t0\t{last}\n"));
    }
    if original.indexes.is_empty() {
        assert!(original.root.is_none());
        return output;
    }

    let td = &original.bytes[7 * PAGE..8 * PAGE];
    assert_eq!(u32_at(td, 51), 1);
    assert_eq!(u32_at(td, 47) as usize, original.indexes.len());
    let history = original.index_history.unwrap();
    assert_eq!(u32_at(td, 67), history);
    // One12-byte count block precedes the original25-byte declarations. The
    // following length-prefixed UTF16 names determine the physical index start.
    let mut physical = 75 + 25 * original.columns.len();
    for column in &original.columns {
        let count = usize::from(u16::from_le_bytes(
            td[physical..physical + 2].try_into().unwrap(),
        ));
        let expected = column
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(count, expected.len());
        assert_eq!(&td[physical + 2..physical + 2 + count], expected);
        physical += 2 + count;
    }
    let root = original.root.unwrap();
    assert_eq!(u32_at(td, physical + 38), root);
    assert_eq!(u32_at(td, physical + 34), (3 << 8) | 16);
    assert_eq!(&td[physical + 4..physical + 6], &[0, 0]);
    assert_eq!(td[physical + 6], 1);
    let flags = td[physical + 46];
    for (number, index) in original.indexes.iter().enumerate() {
        let logical = physical + 52 + number * 28;
        assert_eq!(u32_at(td, logical + 4) as usize, number);
        assert_eq!(u32_at(td, logical + 8), 0);
        let kind = td[logical + 23];
        assert!(matches!(kind, 0 | 1));
        output.push_str(&format!(
            "INDEX\t{name}\t{}\t{number}\t0\t{root}\t{kind}\t{flags:02x}\t{history}\t{}\t0\ttrue\n",
            hex(index.as_bytes()),
            original.index_pages.len()
        ));
    }
    output
}

fn membership(original: &model::Source) -> String {
    let name = hex(original.table.as_bytes());
    let mut order = original
        .rows
        .iter()
        .enumerate()
        .map(|(slot, row)| {
            let key = match row[0] {
                Value::Null => None,
                Value::Int32(value) => Some(value),
                _ => panic!("positive index sources have only actual nullable Int32 keys"),
            };
            (key, slot)
        })
        .collect::<Vec<_>>();
    // This is literal relational order from original cells, not decoding the
    // stored INDEX stream or accepting a reader-produced cursor as an oracle.
    order.sort();
    let mut output = String::new();
    for (number, index) in original.indexes.iter().enumerate() {
        let index_name = hex(index.as_bytes());
        for (ordinal, &(_, slot)) in order.iter().enumerate() {
            output.push_str(&format!(
                "INDEX_ROW\t{name}\t{index_name}\t{ordinal}\t8\t{slot}"
            ));
            for value in &original.rows[slot] {
                output.push('\t');
                output.push_str(&cell(value));
            }
            output.push('\n');
        }
        if number == 0 {
            let mut pages = original.index_pages.clone();
            pages.sort();
            pages.dedup();
            assert_eq!(pages.len(), original.index_pages.len());
            for page in pages {
                let at = page as usize * PAGE;
                output.push_str(&format!(
                    "INDEX_PAGE\t{name}\t0\t{page}\t{}\n",
                    hex(&original.bytes[at..at + PAGE])
                ));
            }
        }
    }
    output
}

pub(in crate::table::mutation::normal_ordinary_sources) fn expected(
    original: &model::Source,
    header: u8,
    digest: &str,
) -> String {
    assert!(original.ordinary);
    let source = BaseSource {
        bytes: original.bytes.clone(),
        tables: vec![Table {
            name: original.table.to_owned(),
            columns: original.columns.clone(),
            rows: original.rows.clone(),
            definition: 7,
            data: vec![8],
        }],
        ordinary: true,
    };
    let baseline = base_expected(&source, header, digest);
    let mut output = String::new();
    let mut declared = false;
    for line in baseline.lines() {
        if !declared && line.starts_with("ROW\t") {
            output.push_str(&declarations(original));
            declared = true;
        }
        if line.starts_with("UNCHANGED\t") {
            assert!(declared);
            output.push_str(&membership(original));
        }
        output.push_str(line);
        output.push('\n');
    }
    output
}

#[test]
#[ignore = "explicit fresh absolute source directory; no automatic export"]
fn export_ordinary_counter_index_sources_and_literal_expectations() {
    let directory =
        env::var_os("TABLECRAFT_ORDINARY_INDEXED_SOURCE_DIR").expect("explicit source directory");
    let directory = Path::new(&directory);
    assert!(directory.is_absolute() && !directory.exists());
    fs::create_dir(directory).unwrap();
    let mut inventory = String::from("id\tordinary\tbytes\tsha256\n");
    let mut inputs = 0;
    let mut reports = 0;
    for header in [5, 6] {
        for case in model::CASES {
            let original = source(header, case);
            let id = format!("h{header}-{}", case.id());
            let digest = hex(&Sha256::digest(&original.bytes));
            create(&directory.join(format!("{id}.accdb")), &original.bytes);
            inventory.push_str(&format!(
                "{id}\t{}\t{}\t{digest}\n",
                original.ordinary,
                original.bytes.len()
            ));
            inputs += 1;
            if original.ordinary {
                create(
                    &directory.join(format!("{id}.expected.tsv")),
                    expected(&original, header, &digest).as_bytes(),
                );
                reports += 1;
            }
        }
    }
    assert_eq!((inputs, reports), (48, 18));
    create(&directory.join("inventory.tsv"), inventory.as_bytes());
    fs::File::open(directory).unwrap().sync_all().unwrap();
}
