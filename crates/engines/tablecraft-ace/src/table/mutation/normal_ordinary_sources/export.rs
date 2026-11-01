//! Explicit source-only export, including independent literal SDK expectations.
use super::{model, source};
use sha2::{Digest, Sha256};
use std::{env, fs, io::Write, path::Path};
use tablecraft_core::values::Value;

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

fn cell(value: &Value) -> String {
    match value {
        Value::Null => "N".into(),
        Value::Boolean(true) => "B1".into(),
        Value::Boolean(false) => "B0".into(),
        Value::Byte(value) => format!("U8:{value}"),
        Value::Int16(value) => format!("I16:{value}"),
        Value::Int32(value) => format!("I32:{value}"),
        Value::Int64(value) => format!("I64:{value}"),
        Value::Currency(value) => format!("C:{}", value.scaled()),
        Value::Single(value) => format!("F32:{:08x}", value.to_bits()),
        Value::Double(value) => format!("F64:{:016x}", value.to_bits()),
        Value::Decimal(value) => format!("DEC:{}:{}", value.coefficient(), value.scale()),
        Value::DateTime(value) => format!("D:{:016x}", value.to_bits()),
        Value::Guid(value) => format!("G:{}", hex(&value.to_bytes())),
        Value::Text(value) => format!("T:{}", hex(value.as_bytes())),
        Value::Binary(value) => format!("X:{}", hex(value)),
        _ => panic!("source has an unqualified SDK carrier"),
    }
}

fn sdk_type(tag: u8) -> &'static str {
    match tag {
        1 => "BOOLEAN",
        2 => "BYTE",
        3 => "INT",
        4 => "LONG",
        5 => "MONEY",
        6 => "FLOAT",
        7 => "DOUBLE",
        8 => "SHORT_DATE_TIME",
        9 => "BINARY",
        10 => "TEXT",
        15 => "GUID",
        16 => "NUMERIC",
        19 => "BIG_INT",
        _ => panic!("source has an unqualified SDK declaration"),
    }
}

fn expected(source: &model::Source, header: u8, digest: &str) -> String {
    let format = if header == 5 { "V2016" } else { "V2019" };
    let mut output = format!(
        "FILE\t{}\t{digest}\nMETADATA\tFILE_FORMAT\tOK\t{format}\nMETADATA\tDATABASE\tOK\t0\nMETADATA\tSUMMARY\tOK\t0\nMETADATA\tUSER_DEFINED\tOK\t0\n",
        source.bytes.len()
    );
    for (name, definition, rows, columns) in [
        ("MSysACEs", 6, 0, 4),
        ("MSysObjects", 2, 7 + source.tables.len(), 15),
        ("MSysRelationships", 5, 0, 8),
    ] {
        output.push_str(&format!(
            "SYSTEM\t{}\t{definition}\t{rows}\t{columns}\n",
            hex(name.as_bytes())
        ));
    }

    let mut tables = source.tables.iter().collect::<Vec<_>>();
    tables.sort_by(|left, right| left.name.cmp(&right.name));
    for table in tables {
        let name = hex(table.name.as_bytes());
        output.push_str(&format!(
            "TABLE\t{name}\t{}\t{}\t{}\n",
            table.definition,
            table.rows.len(),
            table.columns.len()
        ));
        for (position, column) in table.columns.iter().enumerate() {
            // Independent source geometry, never a production descriptor or
            // decoder. The original schemas use 63-byte TD headers and 25-byte
            // declarations, already qualified through earlier source owners.
            let tag = source.bytes[table.definition as usize * 4096 + 63 + position * 25];
            output.push_str(&format!(
                "COLUMN\t{name}\t{position}\t{}\t{}\n",
                hex(column.as_bytes()),
                sdk_type(tag)
            ));
        }

        let mut ordinal = 0;
        for &page in &table.data {
            let base = page as usize * 4096;
            let count = u16::from_le_bytes(source.bytes[base + 12..base + 14].try_into().unwrap());
            for slot in 0..usize::from(count) {
                let row = &table.rows[ordinal];
                output.push_str(&format!("ROW\t{name}\t{ordinal}"));
                for value in row {
                    output.push('\t');
                    output.push_str(&cell(value));
                }
                output.push('\n');

                for (column, value) in row.iter().enumerate() {
                    if let Value::DateTime(value) = value {
                        // Both original signed-zero DATE representations in
                        // this source family denote the same epoch midnight.
                        assert!(matches!(value.to_bits(), 0 | 0x8000_0000_0000_0000));
                        output.push_str(&format!(
                            "DATE\t{name}\t{ordinal}\t{column}\t1899-12-30T00:00\n"
                        ));
                    }
                }

                let word = base + 14 + slot * 2;
                let start = usize::from(u16::from_le_bytes(
                    source.bytes[word..word + 2].try_into().unwrap(),
                ));
                let end = if slot == 0 {
                    4096
                } else {
                    usize::from(u16::from_le_bytes(
                        source.bytes[word - 2..word].try_into().unwrap(),
                    ))
                };
                assert!(start < end && end <= 4096);
                output.push_str(&format!(
                    "RAW\t{name}\t{ordinal}\t{page}\t{slot}\t{}\n",
                    hex(&source.bytes[base + start..base + end])
                ));
                ordinal += 1;
            }
        }
        assert_eq!(ordinal, table.rows.len());
    }
    output.push_str(&format!("UNCHANGED\t{digest}\n"));
    output
}

#[test]
#[ignore = "explicit fresh absolute source directory; no automatic export"]
fn export_ordinary_content_sources_and_literal_expectations() {
    let directory =
        env::var_os("TABLECRAFT_ORDINARY_SOURCE_DIR").expect("explicit source directory");
    let directory = Path::new(&directory);
    assert!(directory.is_absolute() && !directory.exists());
    fs::create_dir(directory).unwrap();
    let mut inventory = String::from("id\tordinary\tbytes\tsha256\n");

    for header in [5, 6] {
        for case in model::CASES {
            let source = source(header, case);
            let id = format!("h{header}-{}", case.id());
            let digest = hex(&Sha256::digest(&source.bytes));
            create(&directory.join(format!("{id}.accdb")), &source.bytes);
            inventory.push_str(&format!(
                "{id}\t{}\t{}\t{digest}\n",
                source.ordinary,
                source.bytes.len()
            ));
            if source.ordinary {
                create(
                    &directory.join(format!("{id}.expected.tsv")),
                    expected(&source, header, &digest).as_bytes(),
                );
            }
        }
    }

    create(&directory.join("inventory.tsv"), inventory.as_bytes());
    fs::File::open(directory).unwrap().sync_all().unwrap();
}
