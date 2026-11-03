//! Existing independent IEEE images as exact original UPDATE requests.
use super::canonical_ordinary_wide_insert as original;
use std::{fs, path::PathBuf};
use tablecraft_core::values::{Double, LegacyDateTime, Single, Value};

const PINS: &[(&str, &str)] = &[
    (
        "h5-fixed-exact-after.accdb",
        "b04d36341ed843e3bf7f2b9bfabd790e9a075d1b8002ad5a53253968c3ea82ed",
    ),
    (
        "h5-fixed-subnormal-after.accdb",
        "8b3cd0ceeda4a9d3875bc8d2d5e2c786097dec4b6b06484ec4ffeb09dce72476",
    ),
    (
        "h5-fixed-special-after.accdb",
        "168914cbc9325e2670576150eddbd23b150f1d1f795a53bdf48c63d642660019",
    ),
    (
        "h6-fixed-exact-after.accdb",
        "1375d5e7453d0603ed42226b05c2f7afa6280f88d63287e2ab73b334a0da339f",
    ),
    (
        "h6-fixed-subnormal-after.accdb",
        "323b381d5484715a0929f29d8451c3a1c2c0c63d02ac9a1978c1148ce713741d",
    ),
    (
        "h6-fixed-special-after.accdb",
        "464904879eaf0b5e70e2a645cac80486497f1b0235cf81fb8e7ae1f1ceb28232",
    ),
];

pub struct Group {
    pub column: &'static str,
    pub expected: Value,
    pub replacement: Value,
}

pub struct Case {
    pub header: u8,
    pub subnormal_to_special: bool,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}

impl Case {
    pub fn groups(&self) -> Vec<Group> {
        let (single_before, single_after, double_before, double_after) =
            if self.subnormal_to_special {
                (1, 0x7f81_2345, 0x000f_ffff_ffff_ffff, 0xfff8_1234_5678_9abc)
            } else {
                (0x8000_0000, 1, 0x8000_0000_0000_0000, 0x000f_ffff_ffff_ffff)
            };
        let mut groups = vec![
            Group {
                column: "SingleValue",
                expected: Value::Single(Single::from_bits(single_before)),
                replacement: Value::Single(Single::from_bits(single_after)),
            },
            Group {
                column: "DoubleValue",
                expected: Value::Double(Double::from_bits(double_before)),
                replacement: Value::Double(Double::from_bits(double_after)),
            },
        ];
        if !self.subnormal_to_special {
            groups.push(Group {
                column: "DateValue",
                expected: Value::DateTime(
                    LegacyDateTime::from_bits(0x8000_0000_0000_0000).unwrap(),
                ),
                replacement: Value::DateTime(LegacyDateTime::from_bits(0).unwrap()),
            });
        }
        groups
    }

    pub fn rows(&self, after: bool) -> Vec<Vec<Value>> {
        let kind = match (self.subnormal_to_special, after) {
            (false, false) => original::Case::FixedExact,
            (false, true) | (true, false) => original::Case::FixedSubnormal,
            (true, true) => original::Case::FixedSpecial,
        };
        let source = original::source(kind);
        let mut rows = source.rows_before;
        rows.push(source.values);
        rows
    }

    pub fn assert_literal_complement(&self) {
        let start = 8 * 4096 + 3907;
        assert_eq!(self.before.len(), 9 * 4096);
        assert_eq!(&self.before[start..start + 2], &10_u16.to_le_bytes());
        assert_eq!(&self.before[start + 61..start + 63], &[0xff, 3]);
        let mut expected = self.before.clone();
        for group in self.groups() {
            let (offset, before, after) = match (group.column, group.expected, group.replacement) {
                ("SingleValue", Value::Single(before), Value::Single(after)) => (
                    25,
                    before.to_bits().to_le_bytes().to_vec(),
                    after.to_bits().to_le_bytes().to_vec(),
                ),
                ("DoubleValue", Value::Double(before), Value::Double(after)) => (
                    29,
                    before.to_bits().to_le_bytes().to_vec(),
                    after.to_bits().to_le_bytes().to_vec(),
                ),
                ("DateValue", Value::DateTime(before), Value::DateTime(after)) => (
                    37,
                    before.to_bits().to_le_bytes().to_vec(),
                    after.to_bits().to_le_bytes().to_vec(),
                ),
                _ => panic!("outside literal IEEE request law"),
            };
            let range = start + offset..start + offset + before.len();
            assert_eq!(&self.before[range.clone()], before);
            expected[range].copy_from_slice(&after);
        }
        assert_eq!(expected, self.after);
    }

    /// Literal original TCNI8 requests; no production codec or staged keys.
    pub fn wire(&self, binding: [u8; 32]) -> Vec<u8> {
        let groups = self.groups();
        let total = 72
            + 5
            + groups
                .iter()
                .map(|group| 12 + group.column.len() + 26)
                .sum::<usize>();
        let mut wire = Vec::from(*b"TCNI0008");
        wire.extend_from_slice(&(total as u32).to_le_bytes());
        wire.extend_from_slice(&[8, 0, 0, 0]);
        wire.extend_from_slice(&(self.before.len() as u64).to_le_bytes());
        wire.extend_from_slice(&binding);
        for number in [5_u32, groups.len() as u32, groups.len() as u32, 0] {
            wire.extend_from_slice(&number.to_le_bytes());
        }
        wire.extend_from_slice(b"Items");
        for group in groups {
            for number in [group.column.len() as u32, 1, 0] {
                wire.extend_from_slice(&number.to_le_bytes());
            }
            wire.extend_from_slice(group.column.as_bytes());
            wire.extend_from_slice(&2_u64.to_le_bytes());
            for value in [group.expected, group.replacement] {
                let (tag, payload) = match value {
                    Value::Single(value) => (6, u64::from(value.to_bits())),
                    Value::Double(value) => (7, value.to_bits()),
                    Value::DateTime(value) => (5, value.to_bits()),
                    _ => panic!("outside literal TCNI8 IEEE/DATE requests"),
                };
                wire.push(tag);
                wire.extend_from_slice(&payload.to_le_bytes());
            }
        }
        assert_eq!(wire.len(), total);
        wire
    }

    pub fn forged_offsets(&self) -> Vec<usize> {
        vec![
            0,
            4096 + 100,
            7 * 4096 + 16,
            8 * 4096 + 2,
            8 * 4096 + 14,
            8 * 4096 + 4033 + 25,
            8 * 4096 + 3907 + 25,
            8 * 4096 + 3907 + 29,
            8 * 4096 + 3907 + 61,
            self.after.len() - 1,
        ]
    }
}

/// Pins all six images before returning any operation case.
pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory = PathBuf::from(std::env::var_os("TABLECRAFT_IEEE_UPDATE_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let images: Vec<_> = PINS
        .iter()
        .map(|(name, hash)| {
            let raw = fs::read(directory.join(name)).unwrap();
            assert_eq!(raw.len(), 36864);
            verify(&raw, hash);
            raw
        })
        .collect();
    let mut cases = Vec::new();
    for (position, header) in [5, 6].into_iter().enumerate() {
        let base = position * 3;
        for (offset, subnormal_to_special) in [(0, false), (1, true)] {
            let case = Case {
                header,
                subnormal_to_special,
                before: images[base + offset].clone(),
                after: images[base + offset + 1].clone(),
            };
            case.assert_literal_complement();
            cases.push(case);
        }
    }
    cases
}
