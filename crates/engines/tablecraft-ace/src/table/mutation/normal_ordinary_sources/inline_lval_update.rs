//! Source-only literal inline Memo/OLE updates; no mutation planner or encoder.
use crate::{
    LongValueCheckStatus, NativeImage, check_table_constraints, check_table_long_values,
    inspect_table_indexes, read_table,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(193);

struct Case {
    name: &'static str,
    memo: Option<&'static str>,
    memo_payload: Option<&'static [u8]>,
    blob: Option<&'static [u8]>,
    id: i32,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "noop",
            memo: Some("old"),
            memo_payload: Some(b"\xff\xfeold"),
            blob: Some(&[199, 0, 255]),
            id: 1,
        },
        Case {
            name: "memo-new",
            memo: Some("new"),
            memo_payload: Some(b"\xff\xfenew"),
            blob: Some(&[199, 0, 255]),
            id: 1,
        },
        Case {
            name: "raw-unicode",
            memo: Some("βγ"),
            memo_payload: Some(&[178, 3, 179, 3]),
            blob: Some(&[199, 0, 255]),
            id: 1,
        },
        Case {
            name: "memo-empty",
            memo: Some(""),
            memo_payload: Some(&[]),
            blob: Some(&[199, 0, 255]),
            id: 1,
        },
        Case {
            name: "memo-clear",
            memo: None,
            memo_payload: None,
            blob: Some(&[199, 0, 255]),
            id: 1,
        },
        Case {
            name: "blob-empty",
            memo: Some("old"),
            memo_payload: Some(b"\xff\xfeold"),
            blob: Some(&[]),
            id: 1,
        },
        Case {
            name: "blob-clear",
            memo: Some("old"),
            memo_payload: Some(b"\xff\xfeold"),
            blob: None,
            id: 1,
        },
        Case {
            name: "both-empty",
            memo: Some(""),
            memo_payload: Some(&[]),
            blob: Some(&[]),
            id: 1,
        },
        Case {
            name: "both-null",
            memo: None,
            memo_payload: None,
            blob: None,
            id: 1,
        },
        Case {
            name: "shared-exact",
            memo: Some("abcdef"),
            memo_payload: Some(b"\xff\xfeabcdef"),
            blob: Some(&[]),
            id: 1,
        },
        Case {
            name: "blob-exact",
            memo: None,
            memo_payload: None,
            blob: Some(&[
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
            ]),
            id: 1,
        },
        Case {
            name: "fixed-joint",
            memo: Some("new"),
            memo_payload: Some(b"\xff\xfenew"),
            blob: Some(&[0, 255, 0]),
            id: 7,
        },
        Case {
            name: "empty-group",
            memo: Some("old"),
            memo_payload: Some(b"\xff\xfeold"),
            blob: Some(&[199, 0, 255]),
            id: 1,
        },
    ]
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn descriptor(payload: Option<&[u8]>) -> Vec<u8> {
    let Some(payload) = payload else {
        return Vec::new();
    };
    assert!(payload.len() <= 64);
    let mut raw = (0x8000_0000_u32 | payload.len() as u32)
        .to_le_bytes()
        .to_vec();
    raw.extend_from_slice(&[0; 8]);
    raw.extend_from_slice(payload);
    raw
}

fn literal_row(original: &[u8], case: &Case) -> Vec<u8> {
    assert_eq!(original.len(), 47);
    let memo = descriptor(case.memo_payload);
    let blob = descriptor(case.blob);
    let end = 6 + memo.len() + blob.len();
    assert!(end <= 38);
    let mut raw = original.to_vec();
    raw[2..6].copy_from_slice(&case.id.to_le_bytes());
    raw[6..6 + memo.len()].copy_from_slice(&memo);
    raw[6 + memo.len()..end].copy_from_slice(&blob);
    for (at, word) in [
        (38, end as u16),
        (40, (6 + memo.len()) as u16),
        (42, 6),
        (44, 2),
    ] {
        raw[at..at + 2].copy_from_slice(&word.to_le_bytes());
    }
    raw[46] = 1 | if case.memo.is_some() { 2 } else { 0 } | if case.blob.is_some() { 4 } else { 0 };
    assert_eq!(&raw[end..38], &original[end..38]);
    raw
}

fn check_view(raw: &[u8], definition: u32, expected: Vec<Value>) {
    let image =
        NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
    let table = read_table(
        &mut image.reader(),
        SCOPE,
        "InlineValues",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.schema().definition_page(), definition);
    assert_eq!(table.rows(), std::slice::from_ref(&expected));
    let fields = table.schema().columns();
    assert_eq!(fields.len(), 3);
    for (field, name, native, flags, extended, width) in [
        (&fields[0], "Id", 4, 3, 0, 4),
        (&fields[1], "MemoValue", 12, 2, 1, 0),
        (&fields[2], "BlobValue", 11, 2, 0, 0),
    ] {
        assert_eq!(
            (
                field.name(),
                field.native_type(),
                field.flags(),
                field.extended_flags(),
                field.storage_length()
            ),
            (name, native, flags, extended, width)
        );
    }
    let constraints = check_table_constraints(
        &mut image.reader(),
        SCOPE,
        "InlineValues",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(constraints.rows(), 1);
    assert!(constraints.relationships().is_empty());
    assert!(
        inspect_table_indexes(
            &mut image.reader(),
            SCOPE,
            "InlineValues",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .indexes()
        .is_empty()
    );
    let long = check_table_long_values(
        &mut image.reader(),
        SCOPE,
        "InlineValues",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(long.rows(), 1);
    assert_eq!(long.columns().len(), 2);
    for (position, column) in long.columns().iter().enumerate() {
        let nulls = usize::from(expected[position + 1] == Value::Null);
        assert_eq!(column.column_number(), position + 1);
        assert_eq!(column.status(), LongValueCheckStatus::Checked);
        assert_eq!(
            (
                column.values().nulls(),
                column.values().inline(),
                column.values().single(),
                column.values().chained()
            ),
            (nulls, 1 - nulls, 0, 0)
        );
        assert_eq!(
            (
                column.owned_pages(),
                column.candidate_pages(),
                column.owned_rows(),
                column.distinct_chunks()
            ),
            (0, 0, 0, 0)
        );
    }
    super::super::normal_ordinary::capture(&image, "InlineValues", &mut budget(), &|| false)
        .unwrap();
    image
        .check_native_ordinary_fixed_table("InlineValues", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(image.as_bytes(), raw);
}

#[test]
#[ignore = "requires independently modeled inline-LVAL UPDATE sources before producers"]
fn inline_lval_update_literals_have_complete_read_and_ordinary_source() {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_INLINE_LVAL_UPDATE_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut contexts = 0;
    for (header, definition, data, bytes, sha) in [
        (
            5,
            89,
            91,
            380928,
            "7a08edca1f32c58ce2451fc6056f257bf7d59fa2483a92b628682d06f5bb9861",
        ),
        (
            6,
            106,
            108,
            450560,
            "3f1bbfc5afcc355ea40a9906ab71a25456fb34e572d15090467211874dff3325",
        ),
    ] {
        let original =
            fs::read(directory.join(format!("h{header}-inline-update-original.accdb"))).unwrap();
        assert_eq!(original.len(), bytes);
        assert_eq!(digest(&original), sha);
        let at = data * 4096 + 4049;
        let old = &original[at..at + 47];
        assert_eq!(&old[..6], &[3, 0, 1, 0, 0, 0]);
        check_view(
            &original,
            definition,
            vec![
                Value::Int32(1),
                Value::Text("old".into()),
                Value::Binary(vec![199, 0, 255]),
            ],
        );
        contexts += 1;
        for case in cases() {
            let actual =
                fs::read(directory.join(format!("h{header}-inline-update-{}.accdb", case.name)))
                    .unwrap();
            let mut expected = original.clone();
            expected[at..at + 47].copy_from_slice(&literal_row(old, &case));
            assert_eq!(
                actual, expected,
                "full literal image {} header{header}",
                case.name
            );
            let values = vec![
                Value::Int32(case.id),
                case.memo
                    .map(|value| Value::Text(value.into()))
                    .unwrap_or(Value::Null),
                case.blob
                    .map(|value| Value::Binary(value.to_vec()))
                    .unwrap_or(Value::Null),
            ];
            check_view(&actual, definition, values);
            contexts += 1;
        }
    }
    assert_eq!(contexts, 28);
}
