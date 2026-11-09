//! Pinned GUID read states, literal records and ordered indexed UPDATE requests.
use std::{fs, path::PathBuf, sync::Arc};
use tablecraft_core::values::{Guid, Value};

pub const PAGE: usize = 4096;
const ENTRY_START: usize = 480;
const CAPACITY: usize = 3616;
pub const RECORD_BYTES: usize = 27;
const MANIFEST_SHA256: &str = "efb835f6b5d4f70760726dc820a6fa388ba78d72462f83586075b081422f6922";

pub const ORIGINAL_PINS: &[(u8, Option<bool>, &str, &str)] = &[
    (
        5,
        None,
        "h5-fixed-exact-after.accdb",
        "b04d36341ed843e3bf7f2b9bfabd790e9a075d1b8002ad5a53253968c3ea82ed",
    ),
    (
        6,
        None,
        "h6-fixed-exact-after.accdb",
        "1375d5e7453d0603ed42226b05c2f7afa6280f88d63287e2ab73b334a0da339f",
    ),
    (
        5,
        Some(false),
        "V2016-Guid-A-pressure-ordinary-source.accdb",
        "4991ffbc914b22d3ae0769e10a7f4c497e8e3066ce8431735a294e620f2a2353",
    ),
    (
        5,
        Some(true),
        "V2016-Guid-D-pressure-ordinary-source.accdb",
        "04abb49839ea396628571f6e47fb563311db061c082634215a8e175dab82f036",
    ),
    (
        6,
        Some(false),
        "V2019-Guid-A-pressure-ordinary-source.accdb",
        "3ccecfaa6865cecdc932bf239dad0bdfeb770361ef90e0d0d149f91979b1837e",
    ),
    (
        6,
        Some(true),
        "V2019-Guid-D-pressure-ordinary-source.accdb",
        "2cd3572d7ec08f5d538a4733d06edc4d8400838a76765bb0a828b343e4dc681d",
    ),
];

#[derive(Clone, Copy)]
pub struct ImageSpec {
    pub header: u8,
    pub descending: bool,
    pub kind: usize,
    pub name: &'static str,
    pub sha256: &'static str,
}

pub struct View {
    pub spec: ImageSpec,
    pub raw: Arc<[u8]>,
}

pub const AFTER_SPECS: &[ImageSpec] = &[
    ImageSpec {
        header: 5,
        descending: false,
        kind: 1,
        name: "h5-A-present-new.accdb",
        sha256: "facebc1c8eda5160f2e5c85905004eb6e663a81af58f2063368fe58b4282f82a",
    },
    ImageSpec {
        header: 5,
        descending: false,
        kind: 2,
        name: "h5-A-present-duplicate.accdb",
        sha256: "910e9963c50ac9631b03beb95572494db5300d42a691d84a5af13bf2360cedac",
    },
    ImageSpec {
        header: 5,
        descending: false,
        kind: 3,
        name: "h5-A-swap-forward.accdb",
        sha256: "9cb20f55f492090358cb0145276c65bad3b60ff1628708eb7744f6f3e884b3be",
    },
    ImageSpec {
        header: 5,
        descending: false,
        kind: 4,
        name: "h5-A-clear-present.accdb",
        sha256: "877c6cee16cd8a9d81950d37d75099bdc97000e1479f472e56bf619b2f6ec89f",
    },
    ImageSpec {
        header: 5,
        descending: false,
        kind: 5,
        name: "h5-A-fill-last-null-new.accdb",
        sha256: "1406d4dbc789c7f4f43716a0f52fbf847a10f89e38fc148c33ebe08392b6a225",
    },
    ImageSpec {
        header: 5,
        descending: false,
        kind: 6,
        name: "h5-A-fill-last-null-duplicate.accdb",
        sha256: "cba526c587d0beb61039c02bd4a643ba0019160180294c785c0d6505b1b52fad",
    },
    ImageSpec {
        header: 5,
        descending: true,
        kind: 1,
        name: "h5-D-present-new.accdb",
        sha256: "14c28cc860fb87c6912f20a47fd1138ae14dd0d00c8cc61bebf734b23b9b13e7",
    },
    ImageSpec {
        header: 5,
        descending: true,
        kind: 2,
        name: "h5-D-present-duplicate.accdb",
        sha256: "0b134668eaebf853a2680e8df947dddc48e14da21adf8f15eb379b3405d7b33a",
    },
    ImageSpec {
        header: 5,
        descending: true,
        kind: 3,
        name: "h5-D-swap-forward.accdb",
        sha256: "9c911d2d9fec9fe8a3e586afa84f3368bd56e5294c4177b5be5f0924d9f70611",
    },
    ImageSpec {
        header: 5,
        descending: true,
        kind: 4,
        name: "h5-D-clear-present.accdb",
        sha256: "9c26740729ad3da7133440e1071988a8f29a6bdc3e633aeccbc2a8c4bf146698",
    },
    ImageSpec {
        header: 5,
        descending: true,
        kind: 5,
        name: "h5-D-fill-last-null-new.accdb",
        sha256: "59e1d0587fdbdf3edac8a709078b90b6d12505512445361c4b71ddb4625146e8",
    },
    ImageSpec {
        header: 5,
        descending: true,
        kind: 6,
        name: "h5-D-fill-last-null-duplicate.accdb",
        sha256: "676b220795387581050d2cfaddc6b31b2e5b5112ef7bed27066fa986eeb5e510",
    },
    ImageSpec {
        header: 6,
        descending: false,
        kind: 1,
        name: "h6-A-present-new.accdb",
        sha256: "ccee3bc037ea1ce82d01b8bb0ccebdff4f6abdcac036ce7a97966f9c1217d367",
    },
    ImageSpec {
        header: 6,
        descending: false,
        kind: 2,
        name: "h6-A-present-duplicate.accdb",
        sha256: "2af0cc160b887ca91b3097d4cb938cf8c2672d2df5ba40594d04d4063a895e56",
    },
    ImageSpec {
        header: 6,
        descending: false,
        kind: 3,
        name: "h6-A-swap-forward.accdb",
        sha256: "b09e2224a07f6055936d8b1e92ac2773645391bc62f9328ff8f995fac4eed6b3",
    },
    ImageSpec {
        header: 6,
        descending: false,
        kind: 4,
        name: "h6-A-clear-present.accdb",
        sha256: "8c7b1d148ae1630c87eb1d72504de64ef76afce1f20cffaef6aa0378d7b011a9",
    },
    ImageSpec {
        header: 6,
        descending: false,
        kind: 5,
        name: "h6-A-fill-last-null-new.accdb",
        sha256: "d18a01be88c243342539ec482a987c53ca146b34683bf2db46645e289e714ae7",
    },
    ImageSpec {
        header: 6,
        descending: false,
        kind: 6,
        name: "h6-A-fill-last-null-duplicate.accdb",
        sha256: "ccb4338c152ccf68977ae5b0c3a727ccfd6adddcfac1865dda164731ccaab6de",
    },
    ImageSpec {
        header: 6,
        descending: true,
        kind: 1,
        name: "h6-D-present-new.accdb",
        sha256: "d1532fce3ecf959ee45cfd3d069cc0cec1c6ec1d50fb69970484c1e14f15e601",
    },
    ImageSpec {
        header: 6,
        descending: true,
        kind: 2,
        name: "h6-D-present-duplicate.accdb",
        sha256: "f065f3d61ef4301b6032ac9172dc0c890b22c74111d72aed7e2320b91b30d405",
    },
    ImageSpec {
        header: 6,
        descending: true,
        kind: 3,
        name: "h6-D-swap-forward.accdb",
        sha256: "d9f92a80b455a73b67a012c0a3928e82d716968174ae948a8603554eb4d182d5",
    },
    ImageSpec {
        header: 6,
        descending: true,
        kind: 4,
        name: "h6-D-clear-present.accdb",
        sha256: "06afabfdca16903d2348ba5d2dbb490cd179280b82e6151814073c42c93d383c",
    },
    ImageSpec {
        header: 6,
        descending: true,
        kind: 5,
        name: "h6-D-fill-last-null-new.accdb",
        sha256: "a36eb4d984a5a4bf0a67759ce5700b64aa9ea560dea3de22461df793ab007f97",
    },
    ImageSpec {
        header: 6,
        descending: true,
        kind: 6,
        name: "h6-D-fill-last-null-duplicate.accdb",
        sha256: "861c31377904078ad5088ec0a480de4d4e60c6d5eed2fbe749ed0d07f29e7ea7",
    },
];

pub fn guid(seed: usize) -> Guid {
    let mut bytes = [0_u8; 16];
    bytes[..4].copy_from_slice(&u32::try_from(seed).unwrap().to_be_bytes());
    bytes[4..].copy_from_slice(&[4, 5, 6, 7, 0x80, 9, 10, 11, 12, 13, 14, 0xff]);
    Guid::from_bytes(bytes)
}

fn native_guid(value: Guid) -> [u8; 16] {
    let mut bytes = value.to_bytes();
    bytes[..4].reverse();
    bytes[4..6].reverse();
    bytes[6..8].reverse();
    bytes
}

pub fn indexed_rows() -> Vec<Vec<Value>> {
    (0..158)
        .map(|slot| {
            vec![
                Value::Int32(slot as i32 + 1),
                if slot == 157 {
                    Value::Null
                } else {
                    Value::Guid(guid(slot))
                },
                Value::Int32(if slot == 157 { 12345 } else { slot as i32 - 64 }),
            ]
        })
        .collect()
}

pub fn geometry(header: u8) -> (usize, [usize; 2], [usize; 2]) {
    match header {
        5 => (89, [93, 95], [91, 92]),
        6 => (106, [111, 113], [108, 109]),
        _ => panic!("outside the two existing native profiles"),
    }
}

pub fn record(raw: &[u8], data: usize, slot: usize, width: usize) -> &[u8] {
    let page = data * PAGE;
    let offset = page + 14 + slot * 2;
    let start = usize::from(u16::from_le_bytes(
        raw[offset..offset + 2].try_into().unwrap(),
    ));
    assert_eq!(start, PAGE - width * (slot + 1));
    &raw[page + start..page + start + width]
}

pub fn indexed_records(raw: &[u8], header: u8) -> Vec<(u32, Vec<u8>)> {
    let (definition, data, _) = geometry(header);
    assert_eq!(
        &raw[definition * PAGE + 16..definition * PAGE + 20],
        &158_u32.to_le_bytes()
    );
    let mut records = Vec::new();
    for (page, count) in [(data[0], 140), (data[1], 18)] {
        assert_eq!(&raw[page * PAGE..page * PAGE + 2], &[1, 1]);
        assert_eq!(
            &raw[page * PAGE + 4..page * PAGE + 8],
            &(definition as u32).to_le_bytes()
        );
        assert_eq!(
            &raw[page * PAGE + 12..page * PAGE + 14],
            &(count as u16).to_le_bytes()
        );
        for slot in 0..count {
            let ordinal = records.len();
            let mut expected = Vec::from(3_u16.to_le_bytes());
            expected.extend_from_slice(&(ordinal as i32 + 1).to_le_bytes());
            // The logical NULL's inactive bytes retain seed156, not zero GUID.
            expected.extend_from_slice(&native_guid(guid(ordinal.min(156))));
            expected.extend_from_slice(
                &(if ordinal == 157 {
                    12345_i32
                } else {
                    ordinal as i32 - 64
                })
                .to_le_bytes(),
            );
            expected.push(if ordinal == 157 { 5 } else { 7 });
            assert_eq!(record(raw, page, slot, 27), expected);
            records.push((((page as u32) << 8) | slot as u32, expected));
        }
    }
    assert_eq!(records.len(), 158);
    records
}

pub fn entries(
    rows: &[Vec<Value>],
    references: &[(u32, Vec<u8>)],
    secondary: bool,
    descending: bool,
) -> Vec<(Vec<u8>, usize)> {
    let mut entries = Vec::new();
    for (slot, row) in rows.iter().enumerate() {
        let mut wire = if secondary {
            match &row[1] {
                Value::Null => vec![if descending { 0xff } else { 0 }],
                Value::Guid(value) => {
                    let mut bytes = value.to_bytes();
                    if descending {
                        for byte in bytes.iter_mut() {
                            *byte = !*byte;
                        }
                    }
                    let mut wire = vec![if descending { 0x80 } else { 0x7f }];
                    wire.extend_from_slice(&bytes[..8]);
                    wire.push(9);
                    wire.extend_from_slice(&bytes[8..]);
                    wire.push(if descending { 0xf7 } else { 8 });
                    wire
                }
                _ => panic!("outside the frozen GUID row law"),
            }
        } else {
            let Value::Int32(id) = row[0] else {
                panic!("manual I32 primary required")
            };
            let mut wire = vec![0x7f];
            wire.extend_from_slice(&((id as u32) ^ 0x8000_0000).to_be_bytes());
            wire
        };
        wire.extend_from_slice(&references[slot].0.to_be_bytes());
        entries.push((wire, slot));
    }
    entries.sort_unstable();
    entries
}

pub fn leaf(raw: &[u8], definition: usize, root: usize, entries: &[(Vec<u8>, usize)]) {
    let page = &raw[root * PAGE..(root + 1) * PAGE];
    assert_eq!(&page[..2], &[4, 1]);
    assert_eq!(&page[4..8], &(definition as u32).to_le_bytes());
    assert_eq!(&page[8..27], &[0; 19]);
    let mut bitmap = [0_u8; 453];
    let mut active = Vec::new();
    for (wire, _) in entries {
        active.extend_from_slice(wire);
        bitmap[active.len() / 8] |= 1 << (active.len() % 8);
    }
    assert_eq!(&page[27..480], &bitmap);
    assert_eq!(&page[480..480 + active.len()], active);
    assert_eq!(
        &page[2..4],
        &u16::try_from(3616 - active.len()).unwrap().to_le_bytes()
    );
}

pub fn new_guid() -> Guid {
    Guid::from_bytes([
        0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0,
    ])
}

pub fn values(kind: usize) -> Vec<Vec<Value>> {
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

pub fn framing(kind: usize) -> (usize, usize, usize) {
    match kind {
        0..=3 => (0, 3616, 0),
        4 => (0, 3598, 18),
        5 => (1, 3477, 139),
        6 => (4, 3006, 610),
        _ => panic!("outside the frozen six outcomes"),
    }
}

pub fn expected(original: &[u8], case: &ImageSpec, references: &[(u32, Vec<u8>)]) -> Vec<u8> {
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

/// Hash every unique complete image before any consumer can inspect a view.
pub fn load_views(mut verify: impl FnMut(&[u8], &str)) -> Vec<View> {
    let original_dir =
        PathBuf::from(std::env::var_os("TABLECRAFT_GUID_UPDATE_INDEX_SOURCE_DIR").unwrap());
    let expected_dir =
        PathBuf::from(std::env::var_os("TABLECRAFT_GUID_INDEXED_UPDATE_EXPECTED_DIR").unwrap());
    assert!(original_dir.is_absolute() && expected_dir.is_absolute());
    verify(
        &fs::read(expected_dir.join("manifest.json")).unwrap(),
        MANIFEST_SHA256,
    );

    let mut specs: Vec<_> = ORIGINAL_PINS
        .iter()
        .filter_map(|&(header, direction, name, sha256)| {
            direction.map(|descending| ImageSpec {
                header,
                descending,
                kind: 0,
                name,
                sha256,
            })
        })
        .collect();
    specs.extend_from_slice(AFTER_SPECS);
    let views: Vec<_> = specs
        .into_iter()
        .map(|spec| {
            let directory = if spec.kind == 0 {
                &original_dir
            } else {
                &expected_dir
            };
            let raw = fs::read(directory.join(spec.name)).unwrap();
            verify(&raw, spec.sha256);
            View {
                spec,
                raw: Arc::from(raw),
            }
        })
        .collect();
    assert_eq!(views.len(), 28);
    for view in &views {
        let original = views
            .iter()
            .find(|candidate| {
                candidate.spec.kind == 0
                    && candidate.spec.header == view.spec.header
                    && candidate.spec.descending == view.spec.descending
            })
            .unwrap();
        let references = indexed_records(&original.raw, original.spec.header);
        assert_eq!(
            view.raw.as_ref(),
            expected(&original.raw, &view.spec, &references)
        );
    }
    views
}
