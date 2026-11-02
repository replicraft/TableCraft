//! Literal storage carriers; actual Source/read qualification, never a planner.
use crate::{
    IndexCheckStatus, NativeImage, check_table_constraints, inspect_table_index_counter,
    read_table_index,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, path::PathBuf};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const SCOPE: CatalogScope = CatalogScope::new(267);
const PAGE: usize = 4096;
const BASE_TEXTS: [Option<&str>; 24] = [
    None,
    Some("Strasse"),
    Some(""),
    Some("a  "),
    Some("a"),
    Some("\u{1f600}"),
    Some("A"),
    Some("\u{0}"),
    Some("\u{e9}"),
    Some(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ),
    Some("e\u{301}"),
    Some("\u{30ab}\u{30bf}\u{30ab}\u{30ca}"),
    Some("Stra\u{df}e"),
    None,
    Some("Strasse"),
    Some(""),
    Some("a  "),
    Some("a"),
    Some("\u{1f600}"),
    Some("A"),
    Some("\u{0}"),
    Some("\u{e9}"),
    Some(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ),
    Some("e\u{301}"),
];

struct Case {
    image: &'static str,
    hash: &'static str,
    seed: &'static str,
    table: &'static str,
    value: &'static str,
    offset: usize,
    record: &'static str,
    indexes: &'static [(&'static str, usize, u32, u32, &'static [usize])],
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(2_000_000_000, 10_000_000, 2_000_000_000)
}

fn number(raw: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(raw[at..at + 2].try_into().unwrap()))
}

fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn unhex(text: &str) -> Vec<u8> {
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[test]
#[ignore = "requires independently authored carrier images and 34 exact SDK reports"]
fn indexed_text_storage_carriers_require_complete_source_and_preserve_raw_capacity_keys_and_history()
 {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_TEXT_STORAGE_CARRIER_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let cases = [
        Case {
            image: "h5-TextLegacyLeaf-compressed-ascii-source.accdb",
            hash: "3197452bdc78ce4efe56f70fc1e85a953a0bf38c4d91e45df5b15a67bb118760",
            seed: "Encoded-V2016-before.accdb",
            table: "TextLegacyLeaf",
            value: "Strasse",
            offset: 389080,
            record: "020002000000fffe5374726173736500730065000f000600010003",
            indexes: &[
                (
                    "Primary",
                    91,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    92,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 2,
                        13, 15, 12,
                    ],
                ),
                (
                    "Twin",
                    92,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 2,
                        13, 15, 12,
                    ],
                ),
                (
                    "Desc",
                    93,
                    2,
                    7,
                    &[
                        12, 2, 13, 15, 9, 11, 22, 24, 10, 23, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h5-TextLegacyLeaf-compressed-ascii-after.accdb",
            hash: "fd661d9662d39ce4507e143c9d67cd3d01f5c0fe914ee13e0def99e5c9ee0101",
            seed: "h5-TextLegacyLeaf-compressed-ascii-source.accdb",
            table: "TextLegacyLeaf",
            value: "a",
            offset: 389080,
            record: "020002000000610053747261737365007300650008000600010003",
            indexes: &[
                (
                    "Primary",
                    91,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    92,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Twin",
                    92,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Desc",
                    93,
                    2,
                    7,
                    &[
                        12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h5-TextLegacyLeaf-compressed-latin1-source.accdb",
            hash: "7dbd46ad4311214b4c67eb3690a7889027a0eb92afdc596b5155fb45fdf42928",
            seed: "Encoded-V2016-before.accdb",
            table: "TextLegacyLeaf",
            value: "\u{e9}",
            offset: 389080,
            record: "020002000000fffee9007200610073007300650009000600010003",
            indexes: &[
                (
                    "Primary",
                    91,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    92,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 4, 5, 7, 17, 18, 20, 10, 23, 2, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Twin",
                    92,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 4, 5, 7, 17, 18, 20, 10, 23, 2, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Desc",
                    93,
                    2,
                    7,
                    &[
                        12, 13, 15, 2, 9, 11, 22, 24, 10, 23, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h5-TextLegacyLeaf-compressed-latin1-after.accdb",
            hash: "e6f3e570433e594dd2258fba4947ac152ce6ef8eca61f32557b42a9f2801f3c4",
            seed: "h5-TextLegacyLeaf-compressed-latin1-source.accdb",
            table: "TextLegacyLeaf",
            value: "a",
            offset: 389080,
            record: "0200020000006100e9007200610073007300650008000600010003",
            indexes: &[
                (
                    "Primary",
                    91,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    92,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Twin",
                    92,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Desc",
                    93,
                    2,
                    7,
                    &[
                        12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h5-TextLegacyLeaf-marked-wide-source.accdb",
            hash: "fb6733676e470623ae59995772610f9a01076bec2d1095ca4c634e1bc8cb9149",
            seed: "Encoded-V2016-before.accdb",
            table: "TextLegacyLeaf",
            value: "a",
            offset: 389080,
            record: "020002000000fffe0061000061007300730065000b000600010003",
            indexes: &[
                (
                    "Primary",
                    91,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    92,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Twin",
                    92,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Desc",
                    93,
                    2,
                    7,
                    &[
                        12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h5-TextLegacyLeaf-marked-wide-after.accdb",
            hash: "1ef6301894117c721171ecdb4b1912c45b6a1e979fb3713eaa174f8a1bc5b604",
            seed: "h5-TextLegacyLeaf-marked-wide-source.accdb",
            table: "TextLegacyLeaf",
            value: "A",
            offset: 389080,
            record: "020002000000410000610000610073007300650008000600010003",
            indexes: &[
                (
                    "Primary",
                    91,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    92,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Twin",
                    92,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Desc",
                    93,
                    2,
                    7,
                    &[
                        12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h5-TextLegacyLeaf-leading-feff-source.accdb",
            hash: "e3f3a05e63ffe63dd1b1104ecbad551ce2fc4874a26b646da56169a78a7f795b",
            seed: "Encoded-V2016-before.accdb",
            table: "TextLegacyLeaf",
            value: "\u{feff}a",
            offset: 389080,
            record: "020002000000fffe00fffe6100007300730065000d000600010003",
            indexes: &[
                (
                    "Primary",
                    91,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    92,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Twin",
                    92,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Desc",
                    93,
                    2,
                    7,
                    &[
                        12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h5-TextLegacyLeaf-leading-feff-after.accdb",
            hash: "6a32143a0400c4e37a699e4709aec86ed690cfb9b51f035d2ad6c6fa3b8d6b97",
            seed: "h5-TextLegacyLeaf-leading-feff-source.accdb",
            table: "TextLegacyLeaf",
            value: "a",
            offset: 389080,
            record: "020002000000610000fffe61000073007300650008000600010003",
            indexes: &[
                (
                    "Primary",
                    91,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    92,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Twin",
                    92,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Desc",
                    93,
                    2,
                    7,
                    &[
                        12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h5-TextModernLeaf-compressed-ascii-source.accdb",
            hash: "4aac1664b707d80ddb67301a8d9d1f658482680bc605d84acf8bc61ea112ab40",
            seed: "Encoded-V2016-before.accdb",
            table: "TextModernLeaf",
            value: "Strasse",
            offset: 417752,
            record: "020002000000fffe5374726173736500730065000f000600010003",
            indexes: &[
                (
                    "Primary",
                    98,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    99,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 2, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Twin",
                    99,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 2, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Desc",
                    100,
                    2,
                    8,
                    &[
                        6, 19, 12, 2, 13, 15, 9, 11, 22, 24, 10, 23, 4, 5, 7, 17, 18, 20, 3, 8, 16,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h5-TextModernLeaf-compressed-ascii-after.accdb",
            hash: "a45c165778416d481e3dc0eb9eef720c3b496c27532d53e4baf720bb49be4d62",
            seed: "h5-TextModernLeaf-compressed-ascii-source.accdb",
            table: "TextModernLeaf",
            value: "a",
            offset: 417752,
            record: "020002000000610053747261737365007300650008000600010003",
            indexes: &[
                (
                    "Primary",
                    98,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    99,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Twin",
                    99,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Desc",
                    100,
                    2,
                    8,
                    &[
                        6, 19, 12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 8, 16,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h5-TextModernLeaf-compressed-latin1-source.accdb",
            hash: "2f1e1ea79efb5195cf97355b4ad53ac80ebb2f241d3b3f1e7e3be5f755186f83",
            seed: "Encoded-V2016-before.accdb",
            table: "TextModernLeaf",
            value: "\u{e9}",
            offset: 417752,
            record: "020002000000fffee9007200610073007300650009000600010003",
            indexes: &[
                (
                    "Primary",
                    98,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    99,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 4, 5, 7, 17, 18, 20, 10, 23, 2, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Twin",
                    99,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 4, 5, 7, 17, 18, 20, 10, 23, 2, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Desc",
                    100,
                    2,
                    8,
                    &[
                        6, 19, 12, 13, 15, 2, 9, 11, 22, 24, 10, 23, 4, 5, 7, 17, 18, 20, 3, 8, 16,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h5-TextModernLeaf-compressed-latin1-after.accdb",
            hash: "33cafc4b94b1516878b641c13440252a15e35f94d57a481db108306053b8bcae",
            seed: "h5-TextModernLeaf-compressed-latin1-source.accdb",
            table: "TextModernLeaf",
            value: "a",
            offset: 417752,
            record: "0200020000006100e9007200610073007300650008000600010003",
            indexes: &[
                (
                    "Primary",
                    98,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    99,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Twin",
                    99,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Desc",
                    100,
                    2,
                    8,
                    &[
                        6, 19, 12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 8, 16,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h5-TextModernLeaf-marked-wide-source.accdb",
            hash: "3bbacbf08408b27c05d8a6cbd9f1e6281c4666670df1c9b69affc4cd39b7392d",
            seed: "Encoded-V2016-before.accdb",
            table: "TextModernLeaf",
            value: "a",
            offset: 417752,
            record: "020002000000fffe0061000061007300730065000b000600010003",
            indexes: &[
                (
                    "Primary",
                    98,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    99,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Twin",
                    99,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Desc",
                    100,
                    2,
                    8,
                    &[
                        6, 19, 12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 8, 16,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h5-TextModernLeaf-marked-wide-after.accdb",
            hash: "f7d026cf4da85a3b3f85cffc7969c92633a883b79675073abff67fc9cd3bac14",
            seed: "h5-TextModernLeaf-marked-wide-source.accdb",
            table: "TextModernLeaf",
            value: "A",
            offset: 417752,
            record: "020002000000410000610000610073007300650008000600010003",
            indexes: &[
                (
                    "Primary",
                    98,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    99,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Twin",
                    99,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Desc",
                    100,
                    2,
                    8,
                    &[
                        6, 19, 12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 8, 16,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h5-TextModernLeaf-leading-feff-source.accdb",
            hash: "3c928f70c3e99c06b48dcf9776b5cba129535b0796b3ef31774a9e3a5a8069f5",
            seed: "Encoded-V2016-before.accdb",
            table: "TextModernLeaf",
            value: "\u{feff}a",
            offset: 417752,
            record: "020002000000fffe00fffe6100007300730065000d000600010003",
            indexes: &[
                (
                    "Primary",
                    98,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    99,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Twin",
                    99,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Desc",
                    100,
                    2,
                    8,
                    &[
                        6, 19, 12, 13, 15, 9, 11, 22, 24, 10, 23, 4, 5, 7, 17, 18, 20, 2, 3, 8, 16,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h5-TextModernLeaf-leading-feff-after.accdb",
            hash: "866da80fa6155a2003b367bee1dbfd593ff665b2f0286c331334fcc0ba85bdcf",
            seed: "h5-TextModernLeaf-leading-feff-source.accdb",
            table: "TextModernLeaf",
            value: "a",
            offset: 417752,
            record: "020002000000610000fffe61000073007300650008000600010003",
            indexes: &[
                (
                    "Primary",
                    98,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    99,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Twin",
                    99,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Desc",
                    100,
                    2,
                    8,
                    &[
                        6, 19, 12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 8, 16,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h6-TextLegacyLeaf-compressed-ascii-source.accdb",
            hash: "30e8040ad121b56b9c8bb5931f1c8569deb915d0c3c8951cdce2d8b903985c77",
            seed: "Encoded-V2019-before.accdb",
            table: "TextLegacyLeaf",
            value: "Strasse",
            offset: 458712,
            record: "020002000000fffe5374726173736500730065000f000600010003",
            indexes: &[
                (
                    "Primary",
                    108,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    109,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 2,
                        13, 15, 12,
                    ],
                ),
                (
                    "Twin",
                    109,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 2,
                        13, 15, 12,
                    ],
                ),
                (
                    "Desc",
                    110,
                    2,
                    7,
                    &[
                        12, 2, 13, 15, 9, 11, 22, 24, 10, 23, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h6-TextLegacyLeaf-compressed-ascii-after.accdb",
            hash: "16c14d3942309b2388601a2fd4dd24515b471ff680243231300e9f6fe64db0d9",
            seed: "h6-TextLegacyLeaf-compressed-ascii-source.accdb",
            table: "TextLegacyLeaf",
            value: "a",
            offset: 458712,
            record: "020002000000610053747261737365007300650008000600010003",
            indexes: &[
                (
                    "Primary",
                    108,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    109,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Twin",
                    109,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Desc",
                    110,
                    2,
                    7,
                    &[
                        12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h6-TextLegacyLeaf-compressed-latin1-source.accdb",
            hash: "fa19b02c948b32ce9a9b7d075f3737e2e659f66e3229c7cbafb99b21092a3f50",
            seed: "Encoded-V2019-before.accdb",
            table: "TextLegacyLeaf",
            value: "\u{e9}",
            offset: 458712,
            record: "020002000000fffee9007200610073007300650009000600010003",
            indexes: &[
                (
                    "Primary",
                    108,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    109,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 4, 5, 7, 17, 18, 20, 10, 23, 2, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Twin",
                    109,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 4, 5, 7, 17, 18, 20, 10, 23, 2, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Desc",
                    110,
                    2,
                    7,
                    &[
                        12, 13, 15, 2, 9, 11, 22, 24, 10, 23, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h6-TextLegacyLeaf-compressed-latin1-after.accdb",
            hash: "e20fae9abc266d7f9d98de3e2aa641f959c83e665f6eb21faffd458d1b6591a3",
            seed: "h6-TextLegacyLeaf-compressed-latin1-source.accdb",
            table: "TextLegacyLeaf",
            value: "a",
            offset: 458712,
            record: "0200020000006100e9007200610073007300650008000600010003",
            indexes: &[
                (
                    "Primary",
                    108,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    109,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Twin",
                    109,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Desc",
                    110,
                    2,
                    7,
                    &[
                        12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h6-TextLegacyLeaf-marked-wide-source.accdb",
            hash: "4472e4070d004a75fa24c81341d62a0b5fe9dac369791d1cb41d46d350a5aebb",
            seed: "Encoded-V2019-before.accdb",
            table: "TextLegacyLeaf",
            value: "a",
            offset: 458712,
            record: "020002000000fffe0061000061007300730065000b000600010003",
            indexes: &[
                (
                    "Primary",
                    108,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    109,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Twin",
                    109,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Desc",
                    110,
                    2,
                    7,
                    &[
                        12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h6-TextLegacyLeaf-marked-wide-after.accdb",
            hash: "15bb053e4332f8ff69aee72e9e7c6877cf8034a1e8dc0db3ce0265520ae2e9a1",
            seed: "h6-TextLegacyLeaf-marked-wide-source.accdb",
            table: "TextLegacyLeaf",
            value: "A",
            offset: 458712,
            record: "020002000000410000610000610073007300650008000600010003",
            indexes: &[
                (
                    "Primary",
                    108,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    109,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Twin",
                    109,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Desc",
                    110,
                    2,
                    7,
                    &[
                        12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h6-TextLegacyLeaf-leading-feff-source.accdb",
            hash: "0b049c5f6746a36f1770488cf09f909bdc0fb28d7efe20c58fe9041f9457e1d2",
            seed: "Encoded-V2019-before.accdb",
            table: "TextLegacyLeaf",
            value: "\u{feff}a",
            offset: 458712,
            record: "020002000000fffe00fffe6100007300730065000d000600010003",
            indexes: &[
                (
                    "Primary",
                    108,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    109,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Twin",
                    109,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Desc",
                    110,
                    2,
                    7,
                    &[
                        12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h6-TextLegacyLeaf-leading-feff-after.accdb",
            hash: "2db196ded69a7f8cba6bb272f7b12b11f89b13ff73ea5ce5997af6979b821277",
            seed: "h6-TextLegacyLeaf-leading-feff-source.accdb",
            table: "TextLegacyLeaf",
            value: "a",
            offset: 458712,
            record: "020002000000610000fffe61000073007300650008000600010003",
            indexes: &[
                (
                    "Primary",
                    108,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    109,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Twin",
                    109,
                    1,
                    7,
                    &[
                        1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24,
                        13, 15, 12,
                    ],
                ),
                (
                    "Desc",
                    110,
                    2,
                    7,
                    &[
                        12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h6-TextModernLeaf-compressed-ascii-source.accdb",
            hash: "2db897619677a9b0e88a74265fd8a273491ac73919d5eb36b94737c4468f030b",
            seed: "Encoded-V2019-before.accdb",
            table: "TextModernLeaf",
            value: "Strasse",
            offset: 487384,
            record: "020002000000fffe5374726173736500730065000f000600010003",
            indexes: &[
                (
                    "Primary",
                    115,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    116,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 2, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Twin",
                    116,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 2, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Desc",
                    117,
                    2,
                    8,
                    &[
                        6, 19, 12, 2, 13, 15, 9, 11, 22, 24, 10, 23, 4, 5, 7, 17, 18, 20, 3, 8, 16,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h6-TextModernLeaf-compressed-ascii-after.accdb",
            hash: "726cd3e699910641302c4f8c9500746ac2a6d98a3626c2d4de66d0dc5b5a30cc",
            seed: "h6-TextModernLeaf-compressed-ascii-source.accdb",
            table: "TextModernLeaf",
            value: "a",
            offset: 487384,
            record: "020002000000610053747261737365007300650008000600010003",
            indexes: &[
                (
                    "Primary",
                    115,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    116,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Twin",
                    116,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Desc",
                    117,
                    2,
                    8,
                    &[
                        6, 19, 12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 8, 16,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h6-TextModernLeaf-compressed-latin1-source.accdb",
            hash: "dd48d91caccdb21b3d5af9ec37413ad80dec5d2d78c4888455cabbbac098d391",
            seed: "Encoded-V2019-before.accdb",
            table: "TextModernLeaf",
            value: "\u{e9}",
            offset: 487384,
            record: "020002000000fffee9007200610073007300650009000600010003",
            indexes: &[
                (
                    "Primary",
                    115,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    116,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 4, 5, 7, 17, 18, 20, 10, 23, 2, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Twin",
                    116,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 4, 5, 7, 17, 18, 20, 10, 23, 2, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Desc",
                    117,
                    2,
                    8,
                    &[
                        6, 19, 12, 13, 15, 2, 9, 11, 22, 24, 10, 23, 4, 5, 7, 17, 18, 20, 3, 8, 16,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h6-TextModernLeaf-compressed-latin1-after.accdb",
            hash: "19f59d855ef7bb41ce07c40d54e6bfa5e2e48797376eb71470e191f39f715357",
            seed: "h6-TextModernLeaf-compressed-latin1-source.accdb",
            table: "TextModernLeaf",
            value: "a",
            offset: 487384,
            record: "0200020000006100e9007200610073007300650008000600010003",
            indexes: &[
                (
                    "Primary",
                    115,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    116,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Twin",
                    116,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Desc",
                    117,
                    2,
                    8,
                    &[
                        6, 19, 12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 8, 16,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h6-TextModernLeaf-marked-wide-source.accdb",
            hash: "921add9223213526d905e9aab91c969af1a7bd6703ebe55a6a2b2a8ebd54f673",
            seed: "Encoded-V2019-before.accdb",
            table: "TextModernLeaf",
            value: "a",
            offset: 487384,
            record: "020002000000fffe0061000061007300730065000b000600010003",
            indexes: &[
                (
                    "Primary",
                    115,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    116,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Twin",
                    116,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Desc",
                    117,
                    2,
                    8,
                    &[
                        6, 19, 12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 8, 16,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h6-TextModernLeaf-marked-wide-after.accdb",
            hash: "bb4381bfd2cf8e6f91016b11a2d34cc28506c99d745fb0b618bd33fec9b3d45c",
            seed: "h6-TextModernLeaf-marked-wide-source.accdb",
            table: "TextModernLeaf",
            value: "A",
            offset: 487384,
            record: "020002000000410000610000610073007300650008000600010003",
            indexes: &[
                (
                    "Primary",
                    115,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    116,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Twin",
                    116,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Desc",
                    117,
                    2,
                    8,
                    &[
                        6, 19, 12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 8, 16,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h6-TextModernLeaf-leading-feff-source.accdb",
            hash: "a08ca59a23701b6dfd3f18069b203db45489da2b3ba61e4304d4f9794055a462",
            seed: "Encoded-V2019-before.accdb",
            table: "TextModernLeaf",
            value: "\u{feff}a",
            offset: 487384,
            record: "020002000000fffe00fffe6100007300730065000d000600010003",
            indexes: &[
                (
                    "Primary",
                    115,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    116,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Twin",
                    116,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Desc",
                    117,
                    2,
                    8,
                    &[
                        6, 19, 12, 13, 15, 9, 11, 22, 24, 10, 23, 4, 5, 7, 17, 18, 20, 2, 3, 8, 16,
                        21, 1, 14,
                    ],
                ),
            ],
        },
        Case {
            image: "h6-TextModernLeaf-leading-feff-after.accdb",
            hash: "14ab1ca3a914306b5dd4a9c63e1317f5201bf34379dc8175cc54d420c0fe2ff8",
            seed: "h6-TextModernLeaf-leading-feff-source.accdb",
            table: "TextModernLeaf",
            value: "a",
            offset: 487384,
            record: "020002000000610000fffe61000073007300650008000600010003",
            indexes: &[
                (
                    "Primary",
                    115,
                    0,
                    24,
                    &[
                        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
                        22, 23, 24,
                    ],
                ),
                (
                    "Asc",
                    116,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Twin",
                    116,
                    1,
                    8,
                    &[
                        1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15,
                        12, 6, 19,
                    ],
                ),
                (
                    "Desc",
                    117,
                    2,
                    8,
                    &[
                        6, 19, 12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 8, 16,
                        21, 1, 14,
                    ],
                ),
            ],
        },
    ];
    assert_eq!(cases.len(), 32);
    for case in cases {
        let raw = fs::read(directory.join(case.image)).unwrap();
        let seed = fs::read(directory.join(case.seed)).unwrap();
        assert_eq!(digest(&raw), case.hash);
        assert_eq!(raw.len(), seed.len());
        let record = unhex(case.record);
        assert_eq!(record.len(), 27);
        assert_eq!(&raw[case.offset..case.offset + 27], record);
        assert_eq!(number(&record, 22), 6);
        assert_eq!(number(&record, 24), 1);
        assert_eq!(record[26], 3);
        let active = number(&record, 20);
        assert!((6..=20).contains(&active));
        for (at, (&before, &after)) in seed.iter().zip(&raw).enumerate() {
            if before == after {
                continue;
            }
            let in_record = (case.offset..case.offset + 27).contains(&at);
            let in_index = case
                .indexes
                .iter()
                .filter(|(_, _, physical, _, _)| *physical != 0)
                .any(|(_, root, _, _, _)| {
                    let base = root * PAGE;
                    let end = PAGE - number(&raw, base + 2);
                    (base + 2..base + 4).contains(&at) || (base + 27..base + end).contains(&at)
                });
            assert!(
                in_record || in_index,
                "unqualified changed absolute byte {at}"
            );
        }
        let image =
            NativeImage::capture(&mut Cursor::new(&raw), SCOPE, &mut budget(), &|| false).unwrap();
        let source = crate::table::mutation::normal_actions::capture(
            &image,
            case.table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let field = &source.table.schema().columns()[1];
        assert_eq!(field.native_type(), 10);
        assert_eq!(field.flags(), 2);
        assert_eq!(field.extended_flags(), 0);
        assert_eq!(field.storage_length(), 510);
        let sort = field.text_sort_order().unwrap();
        assert_eq!((sort.lcid(), sort.variant()), (1033, 0));
        assert_eq!(
            sort.version(),
            if case.table == "TextLegacyLeaf" { 0 } else { 1 }
        );
        let mut wanted = BASE_TEXTS
            .iter()
            .enumerate()
            .map(|(ordinal, text)| {
                vec![
                    Value::Int32(ordinal as i32 + 1),
                    text.map_or(Value::Null, |text| Value::Text(text.into())),
                ]
            })
            .collect::<Vec<_>>();
        wanted[1][1] = Value::Text(case.value.into());
        assert_eq!(source.table.rows(), wanted);
        assert_eq!(source.records.len(), 24);
        assert_eq!(source.records[1].offset, case.offset);
        assert_eq!(source.records[1].data, record);
        let checked = crate::table::indexes::check_table_retained(
            &mut image.reader(),
            SCOPE,
            case.table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(checked.rows(), 24);
        assert_eq!(checked.indexes().len(), 4);
        for &(name, root, physical, history, order) in case.indexes {
            let declaration = checked
                .indexes()
                .iter()
                .find(|entry| entry.index().name() == name)
                .unwrap();
            assert_eq!(declaration.status(), IndexCheckStatus::Checked);
            assert_eq!(declaration.index().root_page() as usize, root);
            assert_eq!(declaration.index().data_number(), physical);
            let found = read_table_index(
                &mut image.reader(),
                SCOPE,
                case.table,
                name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let ordered = order
                .iter()
                .map(|id| wanted[id - 1].clone())
                .collect::<Vec<_>>();
            assert_eq!(found.rows(), ordered);
            let counter = inspect_table_index_counter(
                &mut image.reader(),
                SCOPE,
                case.table,
                name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                (counter.physical_index(), counter.value()),
                (physical, history)
            );
            assert_eq!(&raw[root * PAGE..root * PAGE + 2], &[4, 1]);
            assert_eq!(number(&raw, root * PAGE + 24), 0);
        }
        let constraints = check_table_constraints(
            &mut image.reader(),
            SCOPE,
            case.table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(constraints.rows(), 24);
        assert!(constraints.relationships().is_empty());
        assert_eq!(digest(image.as_bytes()), case.hash);
    }
}
