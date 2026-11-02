//! Independently authored source pins, original presence and physical ROW order.
use std::{env, fs, path::PathBuf};
use tablecraft_core::values::Value;

struct Spec {
    header: u8,
    definition: usize,
    data: [usize; 2],
    roots: [usize; 2],
    bytes: usize,
    before_sha: &'static str,
    after_sha: &'static str,
}

const SPECS: &[Spec] = &[
    Spec {
        header: 5,
        definition: 89,
        data: [93, 95],
        roots: [91, 92],
        bytes: 409600,
        before_sha: "5e88958c40f4c17e213e339fdbbaebc63411929230679770c9bd97eb09c4e483",
        after_sha: "2935b08c9a90e2ea3b9565a01d4765c807363b18c3d3ce591b61a6a73bfca78d",
    },
    Spec {
        header: 6,
        definition: 106,
        data: [110, 112],
        roots: [108, 109],
        bytes: 479232,
        before_sha: "8f092101f25578a11f4ede1130f568b041e42a512701e28206e128f7fe9e888b",
        after_sha: "2df68dbc7402ba00265b86e0f639d63eac655e19786ade20774a8446788d26f7",
    },
];

pub const ALIASES: [&str; 4] = ["PrimaryID", "PrimaryAlias", "GroupOrder", "GroupAlias"];

pub struct Case {
    pub header: u8,
    pub table: &'static str,
    pub definition: usize,
    pub data: [usize; 2],
    pub roots: [usize; 2],
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}

pub fn presence() -> Vec<Vec<Option<Value>>> {
    vec![
        vec![
            None,
            Some(Value::Int32(17)),
            Some(Value::Int32(7)),
            Some(Value::Text("L".repeat(200))),
        ],
        vec![
            None,
            Some(Value::Int32(18)),
            Some(Value::Int32(7)),
            Some(Value::Text("new".into())),
        ],
    ]
}

fn row(id: i32, score: i32, guard: i32, label: String) -> Vec<Value> {
    vec![
        Value::Int32(id),
        Value::Int32(score),
        Value::Int32(guard),
        Value::Text(label),
    ]
}

impl Case {
    pub fn rows(&self, changed: bool) -> Vec<Vec<Value>> {
        let mut rows: Vec<_> = (1..=8)
            .map(|id| row(id, id - 5, (id - 1) % 3, "o".repeat(230)))
            .collect();
        if changed {
            rows.push(row(139, 18, 7, "new".into()));
        }
        rows.push(row(9, 4, 2, "o".repeat(230)));
        if changed {
            rows.push(row(138, 17, 7, "L".repeat(200)));
        }
        rows
    }

    pub fn indexed_rows(&self, alias: &str) -> Vec<Vec<Value>> {
        let mut rows = self.rows(true);
        let column = if alias.starts_with("Primary") { 0 } else { 2 };
        // Stable sort retains physical ROW order for equal Guard7: short 139
        // on the earlier page precedes long 138 on the later page.
        rows.sort_by_key(|row| match row[column] {
            Value::Int32(value) => value,
            _ => unreachable!(),
        });
        rows
    }

    pub fn protected_offsets(&self) -> Vec<usize> {
        let td = self.definition * 4096;
        let early = self.data[0] * 4096;
        let later = self.data[1] * 4096;
        let mut offsets = vec![
            0,
            td + 16,
            td + 20,
            td + 67,
            td + 79,
            td + 102,
            early + 2,
            early + 12,
            early + 30,
            early + 221,
            early + 223,
            early + 229,
            early + 240,
            early + 247,
            early + 248,
            later + 2,
            later + 12,
            later + 16,
            later + 3194,
            later + 3196,
            later + 3202,
            later + 3607,
            later + 3614,
            later + 3615,
            4096 + 100,
            self.before.len() - 1,
        ];
        for root in self.roots {
            offsets.extend([
                root * 4096 + 2,
                root * 4096 + 27,
                root * 4096 + 480,
                root * 4096 + 485,
                root * 4096 + 578,
                root * 4096 + 579,
            ]);
        }
        offsets.sort_unstable();
        offsets.dedup();
        offsets
    }

    pub fn wire(&self, binding: [u8; 32], empty: bool) -> Vec<u8> {
        let rows = if empty { Vec::new() } else { presence() };
        let raw =
            super::tcna2::original_presence_wire(self.table, self.before.len(), binding, rows);
        assert_eq!(raw.len(), if empty { 73 } else { 372 });
        raw
    }
}

pub fn load_all(verify: impl Fn(&[u8], &str)) -> Vec<Case> {
    let directory = PathBuf::from(
        env::var_os("TABLECRAFT_INCREMENT_MULTI_SOURCE_DIR").expect("qualified source directory"),
    );
    assert!(directory.is_absolute());
    let mut cases = Vec::new();
    for spec in SPECS {
        let before =
            fs::read(directory.join(format!("h{}-increment-multi-before.accdb", spec.header)))
                .unwrap();
        let after =
            fs::read(directory.join(format!("h{}-increment-multi-batch.accdb", spec.header)))
                .unwrap();
        assert_eq!(before.len(), spec.bytes);
        assert_eq!(after.len(), spec.bytes);
        verify(&before, spec.before_sha);
        verify(&after, spec.after_sha);
        cases.push(Case {
            header: spec.header,
            table: "Items",
            definition: spec.definition,
            data: spec.data,
            roots: spec.roots,
            before,
            after,
        });
    }
    cases
}
