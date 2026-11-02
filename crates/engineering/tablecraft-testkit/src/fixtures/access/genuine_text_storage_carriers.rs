//! Frozen 32 source/outcome images; no engine or serialization dependency.
use std::{env, fs, path::PathBuf};
use tablecraft_core::values::Value;

pub struct Case {
    pub header: u8,
    pub table: &'static str,
    pub carrier: &'static str,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub expected: &'static str,
    pub value: &'static str,
    pub definition: u32,
    pub data: u32,
    pub roots: [u32; 3],
    pub offset: usize,
    pub histories: [u32; 3],
    orders_before: [&'static [usize]; 4],
    orders_after: [&'static [usize]; 4],
}

impl Case {
    pub const fn changed_rows(&self) -> usize {
        1
    }

    pub fn updates(&self) -> Vec<(usize, Value, Value)> {
        vec![(
            1,
            Value::Text(self.expected.into()),
            Value::Text(self.value.into()),
        )]
    }

    pub fn ordered_rows(&self, after: bool, alias: usize) -> Vec<Vec<Value>> {
        let rows = self.rows(after);
        let order = if after {
            self.orders_after[alias]
        } else {
            self.orders_before[alias]
        };
        order.iter().map(|id| rows[id - 1].clone()).collect()
    }

    pub fn rows(&self, after: bool) -> Vec<Vec<Value>> {
        let mut rows = super::genuine_text_indexed_update::original_rows();
        rows[1][1] = Value::Text(if after { self.value } else { self.expected }.into());
        rows
    }
}

struct Spec {
    source: &'static str,
    source_hash: &'static str,
    after: &'static str,
    after_hash: &'static str,
    table: &'static str,
    carrier: &'static str,
    expected: &'static str,
    value: &'static str,
    header: u8,
    definition: u32,
    data: u32,
    roots: [u32; 3],
    offset: usize,
    histories: [u32; 3],
    orders_before: [&'static [usize]; 4],
    orders_after: [&'static [usize]; 4],
}

pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory =
        PathBuf::from(env::var_os("TABLECRAFT_TEXT_STORAGE_CARRIER_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let specs = [
        Spec {
            source: "h5-TextLegacyLeaf-compressed-ascii-source.accdb",
            source_hash: "3197452bdc78ce4efe56f70fc1e85a953a0bf38c4d91e45df5b15a67bb118760",
            after: "h5-TextLegacyLeaf-compressed-ascii-after.accdb",
            after_hash: "fd661d9662d39ce4507e143c9d67cd3d01f5c0fe914ee13e0def99e5c9ee0101",
            table: "TextLegacyLeaf",
            carrier: "compressed-ascii",
            expected: "Strasse",
            value: "a",
            header: 5,
            definition: 89,
            data: 94,
            roots: [91, 92, 93],
            offset: 389080,
            histories: [24, 7, 7],
            orders_before: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 2, 13,
                    15, 12,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 2, 13,
                    15, 12,
                ],
                &[
                    12, 2, 13, 15, 9, 11, 22, 24, 10, 23, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19, 21,
                    1, 14,
                ],
            ],
            orders_after: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19, 21,
                    1, 14,
                ],
            ],
        },
        Spec {
            source: "h5-TextLegacyLeaf-compressed-latin1-source.accdb",
            source_hash: "7dbd46ad4311214b4c67eb3690a7889027a0eb92afdc596b5155fb45fdf42928",
            after: "h5-TextLegacyLeaf-compressed-latin1-after.accdb",
            after_hash: "e6f3e570433e594dd2258fba4947ac152ce6ef8eca61f32557b42a9f2801f3c4",
            table: "TextLegacyLeaf",
            carrier: "compressed-latin1",
            expected: "\u{e9}",
            value: "a",
            header: 5,
            definition: 89,
            data: 94,
            roots: [91, 92, 93],
            offset: 389080,
            histories: [24, 7, 7],
            orders_before: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 4, 5, 7, 17, 18, 20, 10, 23, 2, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 4, 5, 7, 17, 18, 20, 10, 23, 2, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    12, 13, 15, 2, 9, 11, 22, 24, 10, 23, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19, 21,
                    1, 14,
                ],
            ],
            orders_after: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19, 21,
                    1, 14,
                ],
            ],
        },
        Spec {
            source: "h5-TextLegacyLeaf-marked-wide-source.accdb",
            source_hash: "fb6733676e470623ae59995772610f9a01076bec2d1095ca4c634e1bc8cb9149",
            after: "h5-TextLegacyLeaf-marked-wide-after.accdb",
            after_hash: "1ef6301894117c721171ecdb4b1912c45b6a1e979fb3713eaa174f8a1bc5b604",
            table: "TextLegacyLeaf",
            carrier: "marked-wide",
            expected: "a",
            value: "A",
            header: 5,
            definition: 89,
            data: 94,
            roots: [91, 92, 93],
            offset: 389080,
            histories: [24, 7, 7],
            orders_before: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19, 21,
                    1, 14,
                ],
            ],
            orders_after: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19, 21,
                    1, 14,
                ],
            ],
        },
        Spec {
            source: "h5-TextLegacyLeaf-leading-feff-source.accdb",
            source_hash: "e3f3a05e63ffe63dd1b1104ecbad551ce2fc4874a26b646da56169a78a7f795b",
            after: "h5-TextLegacyLeaf-leading-feff-after.accdb",
            after_hash: "6a32143a0400c4e37a699e4709aec86ed690cfb9b51f035d2ad6c6fa3b8d6b97",
            table: "TextLegacyLeaf",
            carrier: "leading-feff",
            expected: "\u{feff}a",
            value: "a",
            header: 5,
            definition: 89,
            data: 94,
            roots: [91, 92, 93],
            offset: 389080,
            histories: [24, 7, 7],
            orders_before: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19, 21,
                    1, 14,
                ],
            ],
            orders_after: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19, 21,
                    1, 14,
                ],
            ],
        },
        Spec {
            source: "h5-TextModernLeaf-compressed-ascii-source.accdb",
            source_hash: "4aac1664b707d80ddb67301a8d9d1f658482680bc605d84acf8bc61ea112ab40",
            after: "h5-TextModernLeaf-compressed-ascii-after.accdb",
            after_hash: "a45c165778416d481e3dc0eb9eef720c3b496c27532d53e4baf720bb49be4d62",
            table: "TextModernLeaf",
            carrier: "compressed-ascii",
            expected: "Strasse",
            value: "a",
            header: 5,
            definition: 96,
            data: 101,
            roots: [98, 99, 100],
            offset: 417752,
            histories: [24, 8, 8],
            orders_before: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 2, 13, 15, 12,
                    6, 19,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 2, 13, 15, 12,
                    6, 19,
                ],
                &[
                    6, 19, 12, 2, 13, 15, 9, 11, 22, 24, 10, 23, 4, 5, 7, 17, 18, 20, 3, 8, 16, 21,
                    1, 14,
                ],
            ],
            orders_after: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    6, 19, 12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 8, 16, 21,
                    1, 14,
                ],
            ],
        },
        Spec {
            source: "h5-TextModernLeaf-compressed-latin1-source.accdb",
            source_hash: "2f1e1ea79efb5195cf97355b4ad53ac80ebb2f241d3b3f1e7e3be5f755186f83",
            after: "h5-TextModernLeaf-compressed-latin1-after.accdb",
            after_hash: "33cafc4b94b1516878b641c13440252a15e35f94d57a481db108306053b8bcae",
            table: "TextModernLeaf",
            carrier: "compressed-latin1",
            expected: "\u{e9}",
            value: "a",
            header: 5,
            definition: 96,
            data: 101,
            roots: [98, 99, 100],
            offset: 417752,
            histories: [24, 8, 8],
            orders_before: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 4, 5, 7, 17, 18, 20, 10, 23, 2, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 4, 5, 7, 17, 18, 20, 10, 23, 2, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    6, 19, 12, 13, 15, 2, 9, 11, 22, 24, 10, 23, 4, 5, 7, 17, 18, 20, 3, 8, 16, 21,
                    1, 14,
                ],
            ],
            orders_after: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    6, 19, 12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 8, 16, 21,
                    1, 14,
                ],
            ],
        },
        Spec {
            source: "h5-TextModernLeaf-marked-wide-source.accdb",
            source_hash: "3bbacbf08408b27c05d8a6cbd9f1e6281c4666670df1c9b69affc4cd39b7392d",
            after: "h5-TextModernLeaf-marked-wide-after.accdb",
            after_hash: "f7d026cf4da85a3b3f85cffc7969c92633a883b79675073abff67fc9cd3bac14",
            table: "TextModernLeaf",
            carrier: "marked-wide",
            expected: "a",
            value: "A",
            header: 5,
            definition: 96,
            data: 101,
            roots: [98, 99, 100],
            offset: 417752,
            histories: [24, 8, 8],
            orders_before: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    6, 19, 12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 8, 16, 21,
                    1, 14,
                ],
            ],
            orders_after: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    6, 19, 12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 8, 16, 21,
                    1, 14,
                ],
            ],
        },
        Spec {
            source: "h5-TextModernLeaf-leading-feff-source.accdb",
            source_hash: "3c928f70c3e99c06b48dcf9776b5cba129535b0796b3ef31774a9e3a5a8069f5",
            after: "h5-TextModernLeaf-leading-feff-after.accdb",
            after_hash: "866da80fa6155a2003b367bee1dbfd593ff665b2f0286c331334fcc0ba85bdcf",
            table: "TextModernLeaf",
            carrier: "leading-feff",
            expected: "\u{feff}a",
            value: "a",
            header: 5,
            definition: 96,
            data: 101,
            roots: [98, 99, 100],
            offset: 417752,
            histories: [24, 8, 8],
            orders_before: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    6, 19, 12, 13, 15, 9, 11, 22, 24, 10, 23, 4, 5, 7, 17, 18, 20, 2, 3, 8, 16, 21,
                    1, 14,
                ],
            ],
            orders_after: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    6, 19, 12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 8, 16, 21,
                    1, 14,
                ],
            ],
        },
        Spec {
            source: "h6-TextLegacyLeaf-compressed-ascii-source.accdb",
            source_hash: "30e8040ad121b56b9c8bb5931f1c8569deb915d0c3c8951cdce2d8b903985c77",
            after: "h6-TextLegacyLeaf-compressed-ascii-after.accdb",
            after_hash: "16c14d3942309b2388601a2fd4dd24515b471ff680243231300e9f6fe64db0d9",
            table: "TextLegacyLeaf",
            carrier: "compressed-ascii",
            expected: "Strasse",
            value: "a",
            header: 6,
            definition: 106,
            data: 111,
            roots: [108, 109, 110],
            offset: 458712,
            histories: [24, 7, 7],
            orders_before: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 2, 13,
                    15, 12,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 2, 13,
                    15, 12,
                ],
                &[
                    12, 2, 13, 15, 9, 11, 22, 24, 10, 23, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19, 21,
                    1, 14,
                ],
            ],
            orders_after: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19, 21,
                    1, 14,
                ],
            ],
        },
        Spec {
            source: "h6-TextLegacyLeaf-compressed-latin1-source.accdb",
            source_hash: "fa19b02c948b32ce9a9b7d075f3737e2e659f66e3229c7cbafb99b21092a3f50",
            after: "h6-TextLegacyLeaf-compressed-latin1-after.accdb",
            after_hash: "e20fae9abc266d7f9d98de3e2aa641f959c83e665f6eb21faffd458d1b6591a3",
            table: "TextLegacyLeaf",
            carrier: "compressed-latin1",
            expected: "\u{e9}",
            value: "a",
            header: 6,
            definition: 106,
            data: 111,
            roots: [108, 109, 110],
            offset: 458712,
            histories: [24, 7, 7],
            orders_before: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 4, 5, 7, 17, 18, 20, 10, 23, 2, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 4, 5, 7, 17, 18, 20, 10, 23, 2, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    12, 13, 15, 2, 9, 11, 22, 24, 10, 23, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19, 21,
                    1, 14,
                ],
            ],
            orders_after: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19, 21,
                    1, 14,
                ],
            ],
        },
        Spec {
            source: "h6-TextLegacyLeaf-marked-wide-source.accdb",
            source_hash: "4472e4070d004a75fa24c81341d62a0b5fe9dac369791d1cb41d46d350a5aebb",
            after: "h6-TextLegacyLeaf-marked-wide-after.accdb",
            after_hash: "15bb053e4332f8ff69aee72e9e7c6877cf8034a1e8dc0db3ce0265520ae2e9a1",
            table: "TextLegacyLeaf",
            carrier: "marked-wide",
            expected: "a",
            value: "A",
            header: 6,
            definition: 106,
            data: 111,
            roots: [108, 109, 110],
            offset: 458712,
            histories: [24, 7, 7],
            orders_before: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19, 21,
                    1, 14,
                ],
            ],
            orders_after: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19, 21,
                    1, 14,
                ],
            ],
        },
        Spec {
            source: "h6-TextLegacyLeaf-leading-feff-source.accdb",
            source_hash: "0b049c5f6746a36f1770488cf09f909bdc0fb28d7efe20c58fe9041f9457e1d2",
            after: "h6-TextLegacyLeaf-leading-feff-after.accdb",
            after_hash: "2db196ded69a7f8cba6bb272f7b12b11f89b13ff73ea5ce5997af6979b821277",
            table: "TextLegacyLeaf",
            carrier: "leading-feff",
            expected: "\u{feff}a",
            value: "a",
            header: 6,
            definition: 106,
            data: 111,
            roots: [108, 109, 110],
            offset: 458712,
            histories: [24, 7, 7],
            orders_before: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19, 21,
                    1, 14,
                ],
            ],
            orders_after: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    1, 14, 3, 6, 8, 16, 19, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13,
                    15, 12,
                ],
                &[
                    12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 6, 8, 16, 19, 21,
                    1, 14,
                ],
            ],
        },
        Spec {
            source: "h6-TextModernLeaf-compressed-ascii-source.accdb",
            source_hash: "2db897619677a9b0e88a74265fd8a273491ac73919d5eb36b94737c4468f030b",
            after: "h6-TextModernLeaf-compressed-ascii-after.accdb",
            after_hash: "726cd3e699910641302c4f8c9500746ac2a6d98a3626c2d4de66d0dc5b5a30cc",
            table: "TextModernLeaf",
            carrier: "compressed-ascii",
            expected: "Strasse",
            value: "a",
            header: 6,
            definition: 113,
            data: 118,
            roots: [115, 116, 117],
            offset: 487384,
            histories: [24, 8, 8],
            orders_before: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 2, 13, 15, 12,
                    6, 19,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 2, 13, 15, 12,
                    6, 19,
                ],
                &[
                    6, 19, 12, 2, 13, 15, 9, 11, 22, 24, 10, 23, 4, 5, 7, 17, 18, 20, 3, 8, 16, 21,
                    1, 14,
                ],
            ],
            orders_after: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    6, 19, 12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 8, 16, 21,
                    1, 14,
                ],
            ],
        },
        Spec {
            source: "h6-TextModernLeaf-compressed-latin1-source.accdb",
            source_hash: "dd48d91caccdb21b3d5af9ec37413ad80dec5d2d78c4888455cabbbac098d391",
            after: "h6-TextModernLeaf-compressed-latin1-after.accdb",
            after_hash: "19f59d855ef7bb41ce07c40d54e6bfa5e2e48797376eb71470e191f39f715357",
            table: "TextModernLeaf",
            carrier: "compressed-latin1",
            expected: "\u{e9}",
            value: "a",
            header: 6,
            definition: 113,
            data: 118,
            roots: [115, 116, 117],
            offset: 487384,
            histories: [24, 8, 8],
            orders_before: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 4, 5, 7, 17, 18, 20, 10, 23, 2, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 4, 5, 7, 17, 18, 20, 10, 23, 2, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    6, 19, 12, 13, 15, 2, 9, 11, 22, 24, 10, 23, 4, 5, 7, 17, 18, 20, 3, 8, 16, 21,
                    1, 14,
                ],
            ],
            orders_after: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    6, 19, 12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 8, 16, 21,
                    1, 14,
                ],
            ],
        },
        Spec {
            source: "h6-TextModernLeaf-marked-wide-source.accdb",
            source_hash: "921add9223213526d905e9aab91c969af1a7bd6703ebe55a6a2b2a8ebd54f673",
            after: "h6-TextModernLeaf-marked-wide-after.accdb",
            after_hash: "bb4381bfd2cf8e6f91016b11a2d34cc28506c99d745fb0b618bd33fec9b3d45c",
            table: "TextModernLeaf",
            carrier: "marked-wide",
            expected: "a",
            value: "A",
            header: 6,
            definition: 113,
            data: 118,
            roots: [115, 116, 117],
            offset: 487384,
            histories: [24, 8, 8],
            orders_before: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    6, 19, 12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 8, 16, 21,
                    1, 14,
                ],
            ],
            orders_after: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    6, 19, 12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 8, 16, 21,
                    1, 14,
                ],
            ],
        },
        Spec {
            source: "h6-TextModernLeaf-leading-feff-source.accdb",
            source_hash: "a08ca59a23701b6dfd3f18069b203db45489da2b3ba61e4304d4f9794055a462",
            after: "h6-TextModernLeaf-leading-feff-after.accdb",
            after_hash: "14ab1ca3a914306b5dd4a9c63e1317f5201bf34379dc8175cc54d420c0fe2ff8",
            table: "TextModernLeaf",
            carrier: "leading-feff",
            expected: "\u{feff}a",
            value: "a",
            header: 6,
            definition: 113,
            data: 118,
            roots: [115, 116, 117],
            offset: 487384,
            histories: [24, 8, 8],
            orders_before: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    6, 19, 12, 13, 15, 9, 11, 22, 24, 10, 23, 4, 5, 7, 17, 18, 20, 2, 3, 8, 16, 21,
                    1, 14,
                ],
            ],
            orders_after: [
                &[
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                    23, 24,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    1, 14, 3, 8, 16, 21, 2, 4, 5, 7, 17, 18, 20, 10, 23, 9, 11, 22, 24, 13, 15, 12,
                    6, 19,
                ],
                &[
                    6, 19, 12, 13, 15, 9, 11, 22, 24, 10, 23, 2, 4, 5, 7, 17, 18, 20, 3, 8, 16, 21,
                    1, 14,
                ],
            ],
        },
    ];
    let mut result = Vec::new();
    for spec in specs {
        let before = fs::read(directory.join(spec.source)).unwrap();
        let after = fs::read(directory.join(spec.after)).unwrap();
        verify(&before, spec.source_hash);
        verify(&after, spec.after_hash);
        assert_eq!(before.len(), after.len());
        result.push(Case {
            header: spec.header,
            table: spec.table,
            carrier: spec.carrier,
            before,
            after,
            expected: spec.expected,
            value: spec.value,
            definition: spec.definition,
            data: spec.data,
            roots: spec.roots,
            offset: spec.offset,
            histories: spec.histories,
            orders_before: spec.orders_before,
            orders_after: spec.orders_after,
        });
    }
    assert_eq!(result.len(), 16);
    result
}
