//! Eighteen independent images and original numeric requests; no engine codecs.
use std::{fs, path::PathBuf};
use tablecraft_core::values::{Currency, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recipe {
    Zero,
    Null,
    Present,
    Swap,
    Noop,
    Empty,
}

pub struct Case {
    pub header: u8,
    pub table: &'static str,
    pub recipe: Recipe,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub definition: usize,
    pub data: usize,
    pub roots: [usize; 3],
}

impl Case {
    pub fn number(&self, units: i64) -> Value {
        if self.table == "Int64Companions" {
            Value::Int64(units)
        } else {
            Value::Currency(Currency::from_scaled(units))
        }
    }

    pub fn updates(&self, reverse: bool) -> Vec<(usize, Value, Value)> {
        let units = match self.recipe {
            Recipe::Zero => vec![(0, Some(i64::MIN), Some(0))],
            Recipe::Null => vec![(2, Some(i64::MAX), None)],
            Recipe::Present => vec![(1, None, Some(i64::MIN))],
            Recipe::Swap => vec![
                (0, Some(i64::MIN), Some(i64::MAX)),
                (2, Some(i64::MAX), Some(i64::MIN)),
            ],
            Recipe::Noop => vec![(0, Some(i64::MIN), Some(i64::MIN))],
            Recipe::Empty => Vec::new(),
        };
        let mut updates: Vec<_> = units
            .into_iter()
            .map(|(row, old, new)| {
                (
                    row,
                    old.map(|n| self.number(n)).unwrap_or(Value::Null),
                    new.map(|n| self.number(n)).unwrap_or(Value::Null),
                )
            })
            .collect();

        if reverse {
            updates.reverse();
        }

        updates
    }

    pub fn orders(&self) -> &'static [bool] {
        if self.recipe == Recipe::Swap {
            &[false, true]
        } else {
            &[false]
        }
    }

    pub fn changed(&self) -> usize {
        match self.recipe {
            Recipe::Swap => 2,
            Recipe::Noop | Recipe::Empty => 0,
            _ => 1,
        }
    }

    pub fn matched(&self) -> usize {
        self.updates(false).len()
    }

    pub fn original_rows(&self) -> Vec<Vec<Value>> {
        vec![
            vec![
                Value::Int32(1),
                self.number(i64::MIN),
                Value::Int32(17),
                Value::Boolean(true),
                Value::Text("first".into()),
                Value::Binary(vec![0, 255]),
            ],
            vec![
                Value::Int32(2),
                Value::Null,
                Value::Int32(34),
                Value::Boolean(false),
                Value::Text("middle".into()),
                Value::Binary(Vec::new()),
            ],
            vec![
                Value::Int32(3),
                self.number(i64::MAX),
                Value::Int32(51),
                Value::Boolean(true),
                Value::Text("tail".into()),
                Value::Null,
            ],
        ]
    }

    pub fn rows(&self) -> Vec<Vec<Value>> {
        let mut rows = self.original_rows();
        for (row, old, new) in self.updates(false) {
            assert_eq!(rows[row][1], old);
            rows[row][1] = new;
        }

        rows
    }

    /// SQL must actually produce the declared carrier; no Int64/Currency alias.
    pub fn sql(&self) -> Option<String> {
        let assignment = match self.recipe {
            Recipe::Zero if self.table == "Int64Companions" => "0 - 0",
            Recipe::Zero => "CCur(0)",
            Recipe::Null | Recipe::Empty => "NULL",
            Recipe::Noop => "KeyValue",
            Recipe::Present | Recipe::Swap => return None,
        };
        let identity = if self.recipe == Recipe::Null {
            3
        } else if self.recipe == Recipe::Empty {
            99
        } else {
            1
        };
        Some(format!(
            "UPDATE [{}] SET KeyValue={} WHERE Id={}",
            self.table, assignment, identity
        ))
    }

    pub fn wire(&self, binding: [u8; 32], reverse: bool) -> Vec<u8> {
        super::original_fixed_wire::single_column_original_wire(
            self.table,
            "KeyValue",
            &self.updates(reverse),
            self.before.len(),
            binding,
        )
    }

    pub fn forgeries(&self) -> Vec<(usize, u8)> {
        let td = self.definition * 4096;
        let data = self.data * 4096;
        let mut roles = vec![
            (20, 1),
            (td + 16, 1),
            (td + 20, 1),
            (td + 67, 1),
            (td + 79, 1),
            (td + 91, 1),
            (td + 99 + 15, 1),
            (data + 2, 1),
            (data + 12, 1),
            (data + 14, 1),
            (data + 4057 + 2, 1),
            (data + 4057 + 14, 1),
            (data + 4095, 8),
            (data + 4056, 8),
            (data + 4057 + 18, 1),
            (data + 4057 + 28, 1),
            (data + 4057 + 30, 1),
        ];
        roles.extend([
            (self.roots[0] * 4096 + 20, 1),
            (self.roots[0] * 4096 + 480, 1),
            (self.roots[0] * 4096 + 507, 1),
        ]);
        for root in &self.roots[1..] {
            roles.extend([(*root * 4096 + 20, 1), (*root * 4096 + 4095, 1)]);
        }
        for (page, slot) in std::iter::once((1_usize, 0_usize))
            .chain((0..5).map(|slot| (self.definition + 1, slot)))
        {
            let base = page * 4096;
            let at = base + 14 + 2 * slot;
            let start = usize::from(u16::from_le_bytes(
                self.before[at..at + 2].try_into().unwrap(),
            ));
            let end = if slot == 0 {
                4096
            } else {
                usize::from(u16::from_le_bytes(
                    self.before[at - 2..at].try_into().unwrap(),
                ))
            };
            assert_eq!(end - start, 69);
            roles.push((base + start + 5, 1));
        }

        roles.sort_unstable();
        roles.dedup();
        assert_eq!(roles.len(), 30);

        roles
    }

    pub fn assert_preserved(&self) {
        assert_eq!(self.before.len(), self.after.len());
        let data = self.data * 4096;
        let mut allowed = Vec::new();
        for (row, old, new) in self.updates(false) {
            if old == new {
                continue;
            }
            let (start, end) = [(4057, 4096), (4018, 4057), (3983, 4018)][row];
            allowed.push(data + end - 1..data + end);
            if new != Value::Null {
                allowed.push(data + start + 6..data + start + 14);
            }
        }
        if self.changed() != 0 {
            let length = match self.recipe {
                Recipe::Null => 23,
                Recipe::Present => 39,
                _ => 31,
            };
            for root in &self.roots[1..] {
                let base = *root * 4096;
                allowed.extend([base + 2..base + 4, base + 27..base + 480 + length]);
                assert_eq!(&self.after[base + 20..base + 22], &[0, 0]);
                assert_eq!(
                    &self.after[base + 480 + length..base + 4096],
                    &self.before[base + 480 + length..base + 4096]
                );
            }
        }
        for (offset, (old, new)) in self.before.iter().zip(&self.after).enumerate() {
            assert!(old == new || allowed.iter().any(|range| range.contains(&offset)));
        }
    }
}

include!("genuine_wide_ordinary_siblings/pins.rs");
