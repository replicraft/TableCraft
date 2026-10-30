//! File transport retains the existing six-field UPDATE value law.
use super::{Cli, Error, Parser, Value, budget, command, input, updates};
use crate::commands::write::args::Command;
use std::cell::Cell;
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};

fn decode(
    bytes: &[u8],
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<(usize, Value, Value)>, Error> {
    let cells = input::update_file::cells(bytes, work, cancelled)?;
    let groups = updates::parse(
        &cells,
        262144,
        |row, expected, value| (row, expected, value),
        work,
        cancelled,
    )?;
    Ok(groups.into_iter().flat_map(|group| group.rows).collect())
}

#[test]
fn update_file_uses_exact_null_literal_escape_and_quoted_payload_law() {
    let rows = decode(
        b"Items\tMemo\t0\ttext\tnull\tvalue:null\nItems\tMemo\t1\ttext\tvalue:\tvalue:value:null\nItems\tMemo\t2\ttext\t\"old\tline\"\t\"new\nline\"\nItems\tOle\t3\tbinary\t00ff\t0102\n",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(rows[0], (0, Value::Null, Value::Text("null".into())));
    assert_eq!(
        rows[1],
        (
            1,
            Value::Text(String::new()),
            Value::Text("value:null".into())
        )
    );
    assert_eq!(
        rows[2],
        (
            2,
            Value::Text("old\tline".into()),
            Value::Text("new\nline".into())
        )
    );
    assert_eq!(
        rows[3],
        (3, Value::Binary(vec![0, 255]), Value::Binary(vec![1, 2]))
    );
}

#[test]
fn malformed_files_and_unknown_kinds_do_not_become_assignments() {
    for bytes in [
        &b""[..],
        &b"Items\tMemo\t0\ttext\tnull\n"[..],
        &b"Items\tMemo\t0\ttext\tnull\t\"unterminated"[..],
        &b"Items\tMemo\t0\ttext\tnull\t\"closed\"trailing"[..],
        &b"Items\tMemo\t0\ttext\tnull\t\xff"[..],
        &b"Items\tMemo\t0\tmisspelled\tnull\tnull"[..],
        &b"Items\tMemo\tbad\ttext\tnull\tvalue:"[..],
    ] {
        assert!(matches!(
            decode(bytes, &mut budget(), &|| false),
            Err(Error::Input(_))
        ));
    }
}

#[test]
fn complete_transport_and_conversion_share_exact_pools_and_final_cancellation() {
    let bytes = b"Items\tMemo\t0\ttext\tnull\tvalue:null\n";
    let pools = [Resource::Bytes, Resource::Rows, Resource::Steps];
    let ceiling = [1_000_000, 100, 1_000_000];
    let mut baseline = ResourceBudget::new(ceiling[0], ceiling[1], ceiling[2]);
    let calls = Cell::new(0);
    decode(bytes, &mut baseline, &|| {
        calls.set(calls.get() + 1);
        false
    })
    .unwrap();
    let used =
        std::array::from_fn::<_, 3, _>(|index| ceiling[index] - baseline.remaining(pools[index]));
    let mut exact = ResourceBudget::new(used[0], used[1], used[2]);
    decode(bytes, &mut exact, &|| false).unwrap();
    for index in 0..3 {
        let mut limits = used;
        limits[index] -= 1;
        let mut short = ResourceBudget::new(limits[0], limits[1], limits[2]);
        assert!(
            matches!(decode(bytes, &mut short, &|| false), Err(Error::Native(InspectError::Limit(error))) if error.resource == pools[index])
        );
    }

    let total = calls.get();
    calls.set(0);
    let mut cancelled = ResourceBudget::new(used[0], used[1], used[2]);
    assert!(matches!(
        decode(bytes, &mut cancelled, &|| {
            calls.set(calls.get() + 1);
            calls.get() == total
        }),
        Err(Error::Native(InspectError::Cancelled))
    ));
}

#[test]
fn update_requires_exactly_one_request_transport_before_any_open() {
    let prefix = [
        "tablecraft",
        "write",
        "preview-update",
        "missing.accdb",
        "out.accdb",
    ];
    assert!(Cli::try_parse_from(prefix).is_err());
    let mut file = prefix.to_vec();
    file.extend(["--set-file", "requests.tsv", "--long-value"]);
    assert!(matches!(command(&file), Command::Update(_)));
    file.extend(["--set", "Items", "Memo", "0", "text", "null", "value:null"]);
    assert!(Cli::try_parse_from(file).is_err());
}
