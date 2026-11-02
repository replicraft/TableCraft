//! Immutable SDK originals and independently authored two-root promotion AFTERs.
use std::{collections::BTreeMap, fs, path::PathBuf};
use tablecraft_core::values::{Currency, Value};

pub struct Case {
    pub inserted: usize,
    pub header: u8,
    pub table: &'static str,
    pub money: bool,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub definition: usize,
    pub roots: [usize; 3],
    pub data: [usize; 3],
    pub eof: usize,
    pub claims: &'static [(usize, u8)],
}

impl Case {
    pub fn values(&self) -> Vec<Value> {
        vec![
            Value::Int32(401),
            if self.money {
                Value::Currency(Currency::from_scaled(i64::MAX))
            } else {
                Value::Int64(i64::MAX)
            },
            Value::Int32(6817),
        ]
    }

    /// Explicit complete request rows; new key families retain exact typed units.
    pub fn insert_values(&self) -> Vec<Vec<Value>> {
        (0..self.inserted)
            .map(|ordinal| {
                let id = 401 + ordinal as i32;
                vec![
                    Value::Int32(id),
                    if self.money {
                        Value::Currency(Currency::from_scaled(i64::MAX - ordinal as i64))
                    } else {
                        Value::Int64(i64::MAX - ordinal as i64)
                    },
                    Value::Int32(id * 17),
                ]
            })
            .collect()
    }

    pub fn rows(&self, inserted: bool) -> Vec<Vec<Value>> {
        let mut rows: Vec<_> = (1..=400)
            .map(|id| {
                vec![
                    Value::Int32(id),
                    if self.money {
                        Value::Currency(Currency::from_scaled(0))
                    } else {
                        Value::Int64(0)
                    },
                    Value::Int32(id * 17),
                ]
            })
            .collect();
        if inserted {
            rows.extend(self.insert_values());
        }
        rows
    }

    /// Original ROW ties remain ascending even for the descending key.
    pub fn index_rows(&self, alias: &str) -> Vec<Vec<Value>> {
        match alias {
            "PrimaryID" => self.rows(true),
            "Ascending" | "Alias" => {
                let mut rows = self.rows(false);
                rows.extend(self.insert_values().into_iter().rev());
                rows
            }
            "Descending" => {
                let mut rows = self.insert_values();
                rows.extend(self.rows(false));
                rows
            }
            _ => panic!("only frozen original aliases"),
        }
    }

    /// Exact original atomic row sequence, sharing only the fixture wire grammar.
    pub fn wire_rows(&self, binding: [u8; 32], first_id: i32) -> Vec<u8> {
        let mut result = self.wire(binding, first_id, i64::MAX, 6817);
        for ordinal in 1..self.inserted {
            let next = self.wire(
                binding,
                first_id + ordinal as i32,
                i64::MAX - ordinal as i64,
                (401 + ordinal as i32) * 17,
            );
            result.extend_from_slice(&next[68 + self.table.len()..]);
        }
        let length = u32::try_from(result.len()).unwrap();
        result[8..12].copy_from_slice(&length.to_le_bytes());
        result[60..64].copy_from_slice(&(self.inserted as u32).to_le_bytes());
        assert_eq!(result.len(), 68 + self.table.len() + 48 * self.inserted);
        result
    }

    /// Explicit existing scalar conversions; no field-carrier coercion is assumed.
    pub fn sql(&self) -> String {
        assert_eq!(self.inserted, 1, "SQL currently supplies one VALUES row");
        let key = if self.money {
            "CCur(922337203685477) + CCur(0.5807)"
        } else {
            "9223372036854775807"
        };
        format!(
            "INSERT INTO [{}] VALUES (CLng(401), {key}, CLng(6817))",
            self.table
        )
    }

    /// Parsed append-query shape remains explicitly outside current SQL writes.
    pub fn unsupported_batch_sql(&self) -> String {
        assert_eq!(self.inserted, 2);
        let (first, second) = if self.money {
            (
                "CCur(922337203685477) + CCur(0.5807)",
                "CCur(922337203685477) + CCur(0.5806)",
            )
        } else {
            ("9223372036854775807", "9223372036854775806")
        };
        format!(
            "INSERT INTO [{}] SELECT CLng(401), {first}, CLng(6817) UNION ALL SELECT CLng(402), {second}, CLng(6834)",
            self.table,
        )
    }

    /// Complete original request wire, independent of the native intent encoder.
    pub fn wire(&self, before_binding: [u8; 32], id: i32, units: i64, marker: i32) -> Vec<u8> {
        let mut wire = Vec::from(*b"TCNA0004");
        wire.extend_from_slice(&(116_u32 + self.table.len() as u32).to_le_bytes());
        wire.extend_from_slice(&[4, 0, 0, 0]);
        wire.extend_from_slice(&(self.before.len() as u64).to_le_bytes());
        wire.extend_from_slice(&before_binding);
        for word in [self.table.len() as u32, 1, 0] {
            wire.extend_from_slice(&word.to_le_bytes());
        }
        wire.extend_from_slice(self.table.as_bytes());
        wire.extend_from_slice(&[48, 0, 0, 0, 3, 0, 0, 0]);
        wire.extend_from_slice(&[2, 0, 0, 0, 4, 0, 0, 0]);
        wire.extend_from_slice(&id.to_le_bytes());
        wire.extend_from_slice(&[if self.money { 4 } else { 3 }, 0, 0, 0, 8, 0, 0, 0]);
        wire.extend_from_slice(&units.to_le_bytes());
        wire.extend_from_slice(&[2, 0, 0, 0, 4, 0, 0, 0]);
        wire.extend_from_slice(&marker.to_le_bytes());
        assert_eq!(wire.len(), 116 + self.table.len());
        wire
    }

    fn reference(&self, id: usize) -> u32 {
        let ordinal = id - 1;
        let group = if ordinal < 194 {
            0
        } else if ordinal < 388 {
            1
        } else {
            2
        };
        (self.data[group] as u32) << 8 | (ordinal - [0, 194, 388][group]) as u32
    }

    fn index_wire(&self, id: usize, physical: usize) -> Vec<u8> {
        let units = if physical == 0 {
            id as i64
        } else if id <= 400 {
            0
        } else {
            i64::MAX - (id - 401) as i64
        };
        let mut bytes = vec![if physical == 2 { 0x80 } else { 0x7f }];
        let mut value = if physical == 0 {
            ((units as u32) ^ 0x8000_0000).to_be_bytes().to_vec()
        } else {
            ((units as u64) ^ 0x8000_0000_0000_0000)
                .to_be_bytes()
                .to_vec()
        };
        if physical == 2 {
            for byte in &mut value {
                *byte = !*byte;
            }
        }
        bytes.extend(value);
        bytes.extend_from_slice(&self.reference(id).to_be_bytes());
        bytes
    }

    fn require_frame(&self, page: &[u8], ids: &[usize], physical: usize, prefix: usize) {
        assert_eq!(page.len(), 4096);
        assert_eq!(
            u16::from_le_bytes(page[24..26].try_into().unwrap()) as usize,
            prefix
        );
        let ends: Vec<_> = page[27..480]
            .iter()
            .enumerate()
            .flat_map(|(byte, value)| {
                (0..8).filter_map(move |bit| (value & (1 << bit) != 0).then_some(byte * 8 + bit))
            })
            .collect();
        assert_eq!(ends.len(), ids.len());
        let first = self.index_wire(ids[0], physical);
        let mut start = 0;
        for (ordinal, (&id, end)) in ids.iter().zip(ends).enumerate() {
            let mut full = if ordinal == 0 {
                Vec::new()
            } else {
                first[..prefix].to_vec()
            };
            assert!(start < end && end < 3616);
            full.extend_from_slice(&page[480 + start..480 + end]);
            assert_eq!(full, self.index_wire(id, physical));
            start = end;
        }
        assert_eq!(
            u16::from_le_bytes(page[2..4].try_into().unwrap()) as usize,
            3616 - start
        );
    }

    /// Check pinned literal framing before any producer; no production key helper.
    pub fn assert_literal_layout(&self) {
        let count = 400 + self.inserted;
        let td = self.definition * 4096;
        assert_eq!(&self.after[td + 16..td + 20], &(count as u32).to_le_bytes());
        for (physical, old) in [400, 1, 1].into_iter().enumerate() {
            let at = td + 67 + 12 * physical;
            assert_eq!(
                &self.after[at..at + 4],
                &((old + self.inserted) as u32).to_le_bytes()
            );
        }
        let data = self.data[2] * 4096;
        assert_eq!(
            &self.after[data + 12..data + 14],
            &((12 + self.inserted) as u16).to_le_bytes()
        );
        assert_eq!(
            &self.after[data + 2..data + 4],
            &((3830 - self.inserted * 21) as u16).to_le_bytes()
        );
        let primary_prefix = if self.inserted == 1 { 0 } else { 3 };
        let primary_active = count * 9 - primary_prefix * (count - 1);
        let at = self.roots[0] * 4096;
        self.require_frame(
            &self.after[at..at + 4096],
            &(1..=count).collect::<Vec<_>>(),
            0,
            primary_prefix,
        );
        assert_eq!(
            &self.after[at + 480 + primary_active..at + 4096],
            &self.before[at + 480 + primary_active..at + 4096]
        );
        let middle = count / 2;
        for physical in 1..3 {
            let ids: Vec<usize> = if physical == 1 {
                (1..=400).chain((401..=count).rev()).collect()
            } else {
                (401..=count).chain(1..=400).collect()
            };
            assert_eq!(ids.len(), count);
            let fence_id = ids[middle - 1];
            let right = self.eof + (physical - 1) * 2;
            let left = right + 1;
            let prefix = if self.inserted == 157 {
                if physical == 1 { 1 } else { 9 }
            } else {
                0
            };
            for (number, previous, next, population, shared) in [
                (left, 0, right, &ids[..middle], 0),
                (right, left, 0, &ids[middle..], prefix),
            ] {
                let page = &self.after[number * 4096..(number + 1) * 4096];
                assert_eq!(&page[..2], &[4, 1]);
                assert_eq!(&page[4..8], &(self.definition as u32).to_le_bytes());
                assert_eq!(&page[12..16], &(previous as u32).to_le_bytes());
                assert_eq!(&page[16..20], &(next as u32).to_le_bytes());
                self.require_frame(page, population, physical, shared);
                let active = population.len() * 13 - shared * (population.len() - 1);
                assert!(page[480 + active..].iter().all(|byte| *byte == 0));
            }
            let root = self.roots[physical] * 4096;
            assert_eq!(&self.after[root..root + 2], &[3, 1]);
            assert_eq!(self.after[root + 26], 1);
            assert_eq!(
                &self.after[root + 20..root + 24],
                &(right as u32).to_le_bytes()
            );
            assert_eq!(&self.after[root + 24..root + 26], &[0, 0]);
            let mut fence = self.index_wire(fence_id, physical);
            fence.extend_from_slice(&(left as u32).to_be_bytes());
            assert_eq!(&self.after[root + 480..root + 497], fence);
            assert_eq!(
                &self.after[root + 497..root + 4096],
                &self.before[root + 497..root + 4096]
            );
        }
    }

    /// Literal source-bound corruption locations, shared by independent proof and recovery checks.
    pub fn forgery_offsets(&self) -> Vec<usize> {
        let td = self.definition * 4096;
        let data = self.data[2] * 4096;
        let mut offsets = vec![
            td + 16,
            td + 67,
            td + 79,
            td + 91,
            td + 51,
            data + 2,
            data + 12,
            data + 38,
            data + 3849,
            data + 3855,
            data + 3867,
            self.data[0] * 4096 + 4095,
            self.roots[0] * 4096 + 4088,
            self.roots[0] * 4096 + 24,
            self.roots[0] * 4096 + 2,
            self.roots[0] * 4096 + 28,
        ];
        if self.inserted >= 2 {
            offsets.extend([data + 40, data + 3830, data + 3836, data + 3848]);
        }
        if self.inserted == 157 {
            // Fresh right halves exercise actual compacted ROW suffixes.
            offsets.extend([
                data + 350,
                data + 885,
                data + 891,
                data + 903,
                self.eof * 4096 + 24,
                self.eof * 4096 + 501,
                (self.eof + 2) * 4096 + 24,
                (self.eof + 2) * 4096 + 493,
                (self.eof + 2) * 4096 + 497,
            ]);
        }
        for root in &self.roots[1..] {
            offsets.extend([
                root * 4096,
                root * 4096 + 2,
                root * 4096 + 20,
                root * 4096 + 24,
                root * 4096 + 26,
                root * 4096 + 29,
                root * 4096 + 480,
                root * 4096 + 489,
                root * 4096 + 493,
                root * 4096 + 497,
                root * 4096 + 4095,
            ]);
        }
        for page in self.eof..self.eof + 4 {
            offsets.extend([
                page * 4096 + 4,
                page * 4096 + 12,
                page * 4096 + 16,
                page * 4096 + 480,
                page * 4096 + 492,
                page * 4096 + 4095,
            ]);
        }
        offsets.extend(self.claims.iter().map(|claim| claim.0));
        offsets.sort_unstable();
        offsets.dedup();
        offsets
    }
}

/// Every unique file is verified before any case reaches a matching consumer.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory =
        PathBuf::from(std::env::var_os("TABLECRAFT_WIDE_ROOT_PROMOTION_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut files = BTreeMap::new();
    let raw = fs::read(directory.join("h5-CurrencyKeys-promoted.accdb")).unwrap();
    verify(
        &raw,
        "c243983f7355f47eff845fc09d7741ed90de495acea7dc16af3cb89fc20e9d7e",
    );
    files.insert("h5-CurrencyKeys-promoted.accdb", raw);
    let raw = fs::read(directory.join("h5-Int64Keys-promoted.accdb")).unwrap();
    verify(
        &raw,
        "af15c15771be4c1c8d5a34ec82a4ec8f08b350db5816199c63d72ac418b8dacd",
    );
    files.insert("h5-Int64Keys-promoted.accdb", raw);
    let raw = fs::read(directory.join("h5-before.accdb")).unwrap();
    verify(
        &raw,
        "8e976662f1c2ddf2e7616870082a71da3a618017ab0cca6faf78f7fe301350a6",
    );
    files.insert("h5-before.accdb", raw);
    let raw = fs::read(directory.join("h6-CurrencyKeys-promoted.accdb")).unwrap();
    verify(
        &raw,
        "eeb86e6fe43ec6889aba6f24e7f9f756d049d26c56513cb451fe9b1a2d23cf81",
    );
    files.insert("h6-CurrencyKeys-promoted.accdb", raw);
    let raw = fs::read(directory.join("h6-Int64Keys-promoted.accdb")).unwrap();
    verify(
        &raw,
        "3668448927e2f2f55fbb278a630ba4386ac34b022d857713f808a94238bbbec9",
    );
    files.insert("h6-Int64Keys-promoted.accdb", raw);
    let raw = fs::read(directory.join("h6-before.accdb")).unwrap();
    verify(
        &raw,
        "dcb7f14d9eab21dd9f518ddd3aafdc90b06f9325a20b5bbe60cdc44303f39dd7",
    );
    files.insert("h6-before.accdb", raw);
    assert_eq!(files.len(), 6);
    from_images(
        1,
        |header| files[format!("h{header}-before.accdb").as_str()].clone(),
        |header, table| files[format!("h{header}-{table}-promoted.accdb").as_str()].clone(),
    )
}

/// Shared immutable physical facts, after each consumer pins all source images.
pub(super) fn from_images(
    inserted: usize,
    mut before: impl FnMut(u8) -> Vec<u8>,
    mut after: impl FnMut(u8, &str) -> Vec<u8>,
) -> Vec<Case> {
    assert!(matches!(inserted, 1 | 2 | 157));
    let cases = vec![
        Case {
            inserted,
            header: 5,
            table: "Int64Keys",
            money: false,
            before: before(5),
            after: after(5, "Int64Keys"),
            definition: 128,
            roots: [130, 131, 132],
            data: [133, 135, 136],
            eof: 185,
            claims: &[
                (8151, 2),
                (532215, 64),
                (8151, 4),
                (532215, 128),
                (8151, 8),
                (532146, 128),
                (8151, 16),
                (532147, 1),
            ],
        },
        Case {
            inserted,
            header: 5,
            table: "CurrencyKeys",
            money: true,
            before: before(5),
            after: after(5, "CurrencyKeys"),
            definition: 144,
            roots: [146, 147, 148],
            data: [149, 151, 152],
            eof: 185,
            claims: &[
                (8151, 2),
                (597749, 64),
                (8151, 4),
                (597749, 128),
                (8151, 8),
                (597680, 128),
                (8151, 16),
                (597681, 1),
            ],
        },
        Case {
            inserted,
            header: 6,
            table: "Int64Keys",
            money: false,
            before: before(6),
            after: after(6, "Int64Keys"),
            definition: 145,
            roots: [147, 148, 149],
            data: [150, 152, 153],
            eof: 202,
            claims: &[
                (8153, 4),
                (601847, 64),
                (8153, 8),
                (601847, 128),
                (8153, 16),
                (601778, 128),
                (8153, 32),
                (601779, 1),
            ],
        },
        Case {
            inserted,
            header: 6,
            table: "CurrencyKeys",
            money: true,
            before: before(6),
            after: after(6, "CurrencyKeys"),
            definition: 161,
            roots: [163, 164, 165],
            data: [166, 168, 169],
            eof: 202,
            claims: &[
                (8153, 4),
                (667381, 64),
                (8153, 8),
                (667381, 128),
                (8153, 16),
                (667312, 128),
                (8153, 32),
                (667313, 1),
            ],
        },
    ];
    assert_eq!(cases.len(), 4);
    for case in &cases {
        assert_eq!(case.before.len(), case.eof * 4096);
        assert_eq!(case.after.len(), (case.eof + 4) * 4096);
        assert_eq!(case.before[20], case.header);
        assert_eq!(case.after[20], case.header);
    }
    cases
}
