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
        if self.inserted == 2 {
            offsets.extend([data + 40, data + 3830, data + 3836, data + 3848]);
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
    assert!((1..=2).contains(&inserted));
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
