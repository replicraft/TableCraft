//! Independent GUID UPDATE AFTER reads; no planner, writer or journal calls.
use super::{PAGE, SCOPE, budget, entries, geometry, guid, indexed_records, indexed_rows, leaf};
use crate::{
    GuidIndexSelection, IndexCheckStatus, NativeImage, check_table, inspect_table_index_counter,
    inspect_table_schema, read_table, read_table_guid_index, read_table_index,
    table::mutation::{no_required_programs::NoRequiredTablePrograms, normal_ordinary},
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::values::{Guid, Value};

const ENTRY_START: usize = 480;
const CAPACITY: usize = 3616;
const RECORD_BYTES: usize = 27;

const MANIFEST_SHA256: &str = "efb835f6b5d4f70760726dc820a6fa388ba78d72462f83586075b081422f6922";

const CASES: &[Case] = &[
    Case {
        header: 5,
        descending: false,
        kind: 0,
        name: "V2016-Guid-A-pressure-ordinary-source.accdb",
        sha256: "4991ffbc914b22d3ae0769e10a7f4c497e8e3066ce8431735a294e620f2a2353",
    },
    Case {
        header: 5,
        descending: true,
        kind: 0,
        name: "V2016-Guid-D-pressure-ordinary-source.accdb",
        sha256: "04abb49839ea396628571f6e47fb563311db061c082634215a8e175dab82f036",
    },
    Case {
        header: 6,
        descending: false,
        kind: 0,
        name: "V2019-Guid-A-pressure-ordinary-source.accdb",
        sha256: "3ccecfaa6865cecdc932bf239dad0bdfeb770361ef90e0d0d149f91979b1837e",
    },
    Case {
        header: 6,
        descending: true,
        kind: 0,
        name: "V2019-Guid-D-pressure-ordinary-source.accdb",
        sha256: "2cd3572d7ec08f5d538a4733d06edc4d8400838a76765bb0a828b343e4dc681d",
    },
    Case {
        header: 5,
        descending: false,
        kind: 1,
        name: "h5-A-present-new.accdb",
        sha256: "facebc1c8eda5160f2e5c85905004eb6e663a81af58f2063368fe58b4282f82a",
    },
    Case {
        header: 5,
        descending: false,
        kind: 2,
        name: "h5-A-present-duplicate.accdb",
        sha256: "910e9963c50ac9631b03beb95572494db5300d42a691d84a5af13bf2360cedac",
    },
    Case {
        header: 5,
        descending: false,
        kind: 3,
        name: "h5-A-swap-forward.accdb",
        sha256: "9cb20f55f492090358cb0145276c65bad3b60ff1628708eb7744f6f3e884b3be",
    },
    Case {
        header: 5,
        descending: false,
        kind: 4,
        name: "h5-A-clear-present.accdb",
        sha256: "877c6cee16cd8a9d81950d37d75099bdc97000e1479f472e56bf619b2f6ec89f",
    },
    Case {
        header: 5,
        descending: false,
        kind: 5,
        name: "h5-A-fill-last-null-new.accdb",
        sha256: "1406d4dbc789c7f4f43716a0f52fbf847a10f89e38fc148c33ebe08392b6a225",
    },
    Case {
        header: 5,
        descending: false,
        kind: 6,
        name: "h5-A-fill-last-null-duplicate.accdb",
        sha256: "cba526c587d0beb61039c02bd4a643ba0019160180294c785c0d6505b1b52fad",
    },
    Case {
        header: 5,
        descending: true,
        kind: 1,
        name: "h5-D-present-new.accdb",
        sha256: "14c28cc860fb87c6912f20a47fd1138ae14dd0d00c8cc61bebf734b23b9b13e7",
    },
    Case {
        header: 5,
        descending: true,
        kind: 2,
        name: "h5-D-present-duplicate.accdb",
        sha256: "0b134668eaebf853a2680e8df947dddc48e14da21adf8f15eb379b3405d7b33a",
    },
    Case {
        header: 5,
        descending: true,
        kind: 3,
        name: "h5-D-swap-forward.accdb",
        sha256: "9c911d2d9fec9fe8a3e586afa84f3368bd56e5294c4177b5be5f0924d9f70611",
    },
    Case {
        header: 5,
        descending: true,
        kind: 4,
        name: "h5-D-clear-present.accdb",
        sha256: "9c26740729ad3da7133440e1071988a8f29a6bdc3e633aeccbc2a8c4bf146698",
    },
    Case {
        header: 5,
        descending: true,
        kind: 5,
        name: "h5-D-fill-last-null-new.accdb",
        sha256: "59e1d0587fdbdf3edac8a709078b90b6d12505512445361c4b71ddb4625146e8",
    },
    Case {
        header: 5,
        descending: true,
        kind: 6,
        name: "h5-D-fill-last-null-duplicate.accdb",
        sha256: "676b220795387581050d2cfaddc6b31b2e5b5112ef7bed27066fa986eeb5e510",
    },
    Case {
        header: 6,
        descending: false,
        kind: 1,
        name: "h6-A-present-new.accdb",
        sha256: "ccee3bc037ea1ce82d01b8bb0ccebdff4f6abdcac036ce7a97966f9c1217d367",
    },
    Case {
        header: 6,
        descending: false,
        kind: 2,
        name: "h6-A-present-duplicate.accdb",
        sha256: "2af0cc160b887ca91b3097d4cb938cf8c2672d2df5ba40594d04d4063a895e56",
    },
    Case {
        header: 6,
        descending: false,
        kind: 3,
        name: "h6-A-swap-forward.accdb",
        sha256: "b09e2224a07f6055936d8b1e92ac2773645391bc62f9328ff8f995fac4eed6b3",
    },
    Case {
        header: 6,
        descending: false,
        kind: 4,
        name: "h6-A-clear-present.accdb",
        sha256: "8c7b1d148ae1630c87eb1d72504de64ef76afce1f20cffaef6aa0378d7b011a9",
    },
    Case {
        header: 6,
        descending: false,
        kind: 5,
        name: "h6-A-fill-last-null-new.accdb",
        sha256: "d18a01be88c243342539ec482a987c53ca146b34683bf2db46645e289e714ae7",
    },
    Case {
        header: 6,
        descending: false,
        kind: 6,
        name: "h6-A-fill-last-null-duplicate.accdb",
        sha256: "ccb4338c152ccf68977ae5b0c3a727ccfd6adddcfac1865dda164731ccaab6de",
    },
    Case {
        header: 6,
        descending: true,
        kind: 1,
        name: "h6-D-present-new.accdb",
        sha256: "d1532fce3ecf959ee45cfd3d069cc0cec1c6ec1d50fb69970484c1e14f15e601",
    },
    Case {
        header: 6,
        descending: true,
        kind: 2,
        name: "h6-D-present-duplicate.accdb",
        sha256: "f065f3d61ef4301b6032ac9172dc0c890b22c74111d72aed7e2320b91b30d405",
    },
    Case {
        header: 6,
        descending: true,
        kind: 3,
        name: "h6-D-swap-forward.accdb",
        sha256: "d9f92a80b455a73b67a012c0a3928e82d716968174ae948a8603554eb4d182d5",
    },
    Case {
        header: 6,
        descending: true,
        kind: 4,
        name: "h6-D-clear-present.accdb",
        sha256: "06afabfdca16903d2348ba5d2dbb490cd179280b82e6151814073c42c93d383c",
    },
    Case {
        header: 6,
        descending: true,
        kind: 5,
        name: "h6-D-fill-last-null-new.accdb",
        sha256: "a36eb4d984a5a4bf0a67759ce5700b64aa9ea560dea3de22461df793ab007f97",
    },
    Case {
        header: 6,
        descending: true,
        kind: 6,
        name: "h6-D-fill-last-null-duplicate.accdb",
        sha256: "861c31377904078ad5088ec0a480de4d4e60c6d5eed2fbe749ed0d07f29e7ea7",
    },
];

struct Case {
    header: u8,
    descending: bool,
    kind: usize,
    name: &'static str,
    sha256: &'static str,
}

fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn new_guid() -> Guid {
    Guid::from_bytes([
        0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0,
    ])
}

fn values(kind: usize) -> Vec<Vec<Value>> {
    let mut rows = indexed_rows();
    match kind {
        0 => {}
        1 => rows[0][1] = Value::Guid(new_guid()),
        2 => rows[0][1] = Value::Guid(guid(1)),
        3 => {
            rows[0][1] = Value::Guid(guid(1));
            rows[1][1] = Value::Guid(guid(0));
        }
        4 => rows[0][1] = Value::Null,
        5 => rows[157][1] = Value::Guid(new_guid()),
        6 => rows[157][1] = Value::Guid(guid(156)),
        _ => panic!("outside the frozen six outcomes"),
    }
    rows
}

fn framing(kind: usize) -> (usize, usize, usize) {
    match kind {
        0..=3 => (0, 3616, 0),
        4 => (0, 3598, 18),
        5 => (1, 3477, 139),
        6 => (4, 3006, 610),
        _ => panic!("outside the frozen six outcomes"),
    }
}

fn expected(original: &[u8], case: &Case, references: &[(u32, Vec<u8>)]) -> Vec<u8> {
    if case.kind == 0 {
        return original.to_vec();
    }
    let rows = values(case.kind);
    let before = indexed_rows();
    let mut result = original.to_vec();
    for (ordinal, row) in rows.iter().enumerate() {
        if row[1] == before[ordinal][1] {
            continue;
        }
        let reference = references[ordinal].0;
        let page = usize::try_from(reference >> 8).unwrap();
        let slot = usize::try_from(reference & 255).unwrap();
        let start = page * PAGE + PAGE - RECORD_BYTES * (slot + 1);
        match row[1] {
            Value::Null => result[start + 26] &= 253,
            Value::Guid(value) => {
                // Independent DATA permutation, not a production scalar codec.
                let mut bytes = value.to_bytes();
                bytes[..4].reverse();
                bytes[4..6].reverse();
                bytes[6..8].reverse();
                result[start + 6..start + 22].copy_from_slice(&bytes);
                result[start + 26] |= 2;
            }
            _ => panic!("outside exact native GUID carriers"),
        }
    }

    let (_, _, roots) = geometry(case.header);
    let start = roots[1] * PAGE;
    let (prefix, length, free) = framing(case.kind);
    let ordered = entries(&rows, references, true, case.descending);
    let mut active = Vec::new();
    let mut bitmap = [0_u8; 453];
    for (position, (wire, _)) in ordered.iter().enumerate() {
        let shared = if position == 0 { 0 } else { prefix };
        active.extend_from_slice(&wire[shared..]);
        let end = active.len();
        bitmap[end / 8] |= 1 << (end % 8);
    }
    assert_eq!(active.len(), length);
    assert_eq!(free + length, CAPACITY);
    result[start + 2..start + 4].copy_from_slice(&u16::try_from(free).unwrap().to_le_bytes());
    result[start + 24..start + 26].copy_from_slice(&u16::try_from(prefix).unwrap().to_le_bytes());
    result[start + 27..start + ENTRY_START].copy_from_slice(&bitmap);
    result[start + ENTRY_START..start + ENTRY_START + length].copy_from_slice(&active);
    assert_eq!(
        &result[start + ENTRY_START + length..start + PAGE],
        &original[start + ENTRY_START + length..start + PAGE],
    );
    result
}

fn inspect(case: &Case, raw: &[u8], original: &[u8], references: &[(u32, Vec<u8>)]) {
    assert_eq!(raw, expected(original, case, references));
    let rows = values(case.kind);
    let (definition, _, roots) = geometry(case.header);
    let primary = entries(&rows, references, false, false);
    leaf(raw, definition, roots[0], &primary);
    for offset in [67, 79] {
        let at = definition * PAGE + offset;
        assert_eq!(&raw[at..at + 4], &158_u32.to_le_bytes());
    }

    let image =
        NativeImage::capture(&mut Cursor::new(raw), SCOPE, &mut budget(), &|| false).unwrap();
    let schema = inspect_table_schema(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(schema.definition_page() as usize, definition);
    assert_eq!(schema.columns().len(), 3);
    for (position, field) in schema.columns().iter().enumerate() {
        assert_eq!(field.number(), position);
        assert_eq!(field.name(), ["ID", "Group", "Score"][position]);
        assert_eq!(field.native_type(), [4, 15, 4][position]);
        assert_eq!(field.storage_length(), [4, 16, 4][position]);
        assert!(field.fixed());
        assert_eq!((field.flags(), field.extended_flags()), (3, 0));
    }
    let read = read_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(read.rows(), rows);
    let checked = check_table(&mut image.reader(), SCOPE, "Items", &mut budget(), &|| {
        false
    })
    .unwrap();
    assert_eq!(checked.rows(), 158);
    assert_eq!(checked.indexes().len(), 4);
    let mut maps = [None; 2];
    let mut aliases = [0; 2];
    let secondary = entries(&rows, references, true, case.descending);
    for checked in checked.indexes() {
        assert_eq!(checked.status(), IndexCheckStatus::Checked);
        let index = checked.index();
        let position = match index.name() {
            "PrimaryID" | "PrimaryAlias" => 0,
            "GroupOrder" | "GroupAlias" => 1,
            _ => panic!("outside the four frozen aliases"),
        };
        aliases[position] += 1;
        assert_eq!(index.data_number(), position as u32);
        assert_eq!(index.root_page() as usize, roots[position]);
        assert_eq!(index.columns().len(), 1);
        assert_eq!(index.columns()[0].number(), position);
        assert_eq!(
            index.columns()[0].descending(),
            position == 1 && case.descending
        );
        if let Some(prior) = maps[position] {
            assert_eq!(index.usage_map(), prior);
        } else {
            maps[position] = Some(index.usage_map());
        }
        let flags = index.constraints().unwrap();
        assert_eq!(flags.primary_key(), index.name() == "PrimaryID");
        assert_eq!(flags.required(), position == 0);
        assert_eq!(flags.unique(), position == 0);
        assert!(!flags.ignore_nulls());
        let order = if position == 0 { &primary } else { &secondary };
        let observed = if position == 0 {
            read_table_index(
                &mut image.reader(),
                SCOPE,
                "Items",
                index.name(),
                &mut budget(),
                &|| false,
            )
        } else {
            read_table_guid_index(
                &mut image.reader(),
                SCOPE,
                "Items",
                index.name(),
                GuidIndexSelection::All,
                &mut budget(),
                &|| false,
            )
        }
        .unwrap();
        let ordered_rows: Vec<_> = order
            .iter()
            .map(|(_, ordinal)| rows[*ordinal].clone())
            .collect();
        assert_eq!(observed.rows(), ordered_rows);
        let history = inspect_table_index_counter(
            &mut image.reader(),
            SCOPE,
            "Items",
            index.name(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(history.value(), 158);

        if position == 1 {
            let nulls = read_table_guid_index(
                &mut image.reader(),
                SCOPE,
                "Items",
                index.name(),
                GuidIndexSelection::Null,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let expected_nulls: Vec<_> = order
                .iter()
                .filter(|(_, ordinal)| rows[*ordinal][1] == Value::Null)
                .map(|(_, ordinal)| rows[*ordinal].clone())
                .collect();
            assert_eq!(nulls.rows(), expected_nulls);
            for selected in [guid(1), guid(156), new_guid()] {
                let equal = read_table_guid_index(
                    &mut image.reader(),
                    SCOPE,
                    "Items",
                    index.name(),
                    GuidIndexSelection::Equal(selected),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let expected_equal: Vec<_> = order
                    .iter()
                    .filter(|(_, ordinal)| rows[*ordinal][1] == Value::Guid(selected))
                    .map(|(_, ordinal)| rows[*ordinal].clone())
                    .collect();
                assert_eq!(equal.rows(), expected_equal);
            }
        }
    }
    assert_eq!(aliases, [2, 2]);
    assert_ne!(maps[0], maps[1]);
    NoRequiredTablePrograms::capture_updates(&image, &schema, &mut budget(), &|| false).unwrap();
    let source = normal_ordinary::capture(&image, "Items", &mut budget(), &|| false).unwrap();
    assert_eq!(source.table.rows(), rows);
    assert_eq!(source.records.len(), 158);
    for (record, (reference, _)) in source.records.iter().zip(references) {
        assert_eq!(record.reference, *reference);
        let page = usize::try_from(reference >> 8).unwrap();
        let slot = usize::try_from(reference & 255).unwrap();
        let start = page * PAGE + PAGE - RECORD_BYTES * (slot + 1);
        assert_eq!(record.data, raw[start..start + RECORD_BYTES]);
    }
    assert_eq!(image.as_bytes(), raw);
}

#[test]
#[ignore = "four original and24 independent GUID UPDATE AFTER reads; no mutation"]
fn independently_authored_indexed_guid_updates_keep_complete_source_and_membership() {
    let originals =
        PathBuf::from(std::env::var_os("TABLECRAFT_GUID_UPDATE_INDEX_SOURCE_DIR").unwrap());
    let expected =
        PathBuf::from(std::env::var_os("TABLECRAFT_GUID_INDEXED_UPDATE_EXPECTED_DIR").unwrap());
    assert!(originals.is_absolute() && expected.is_absolute());
    assert_eq!(
        digest(&fs::read(expected.join("manifest.json")).unwrap()),
        MANIFEST_SHA256
    );
    // Every unique complete-image pin is checked before any native inspection.
    let images: Vec<_> = CASES
        .iter()
        .map(|case| {
            let directory = if case.kind == 0 {
                &originals
            } else {
                &expected
            };
            let raw = fs::read(directory.join(case.name)).unwrap();
            assert_eq!(digest(&raw), case.sha256);
            raw
        })
        .collect();
    let original_records: Vec<_> = CASES
        .iter()
        .enumerate()
        .filter(|(_, case)| case.kind == 0)
        .map(|(position, case)| (position, indexed_records(&images[position], case.header)))
        .collect();
    assert_eq!(original_records.len(), 4);

    for (case, raw) in CASES.iter().zip(&images) {
        let (position, references) = original_records
            .iter()
            .find(|(position, _)| {
                let original = &CASES[*position];
                original.header == case.header && original.descending == case.descending
            })
            .unwrap();
        inspect(case, raw, &images[*position], references);
    }
    assert_eq!(images.len(), 28);
}
