//! Pure original/AFTER pins and explicit presence values; no engine or generator.
use std::{env, fs, path::PathBuf};
use tablecraft_core::values::Value;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Text,
    Binary,
    Fixed,
    Middle,
}

struct Spec {
    id: &'static str,
    before_name: &'static str,
    before_sha: &'static str,
    after_sha: &'static str,
    family: Family,
    state: &'static str,
    definition: u32,
    data: u32,
    root: u32,
    generated: i32,
    generated_position: usize,
    old_last: i32,
    old_live: usize,
    old_slots: usize,
    new_start: usize,
    record_bytes: usize,
    history: u32,
}

const SPECS: &[Spec] = &[
    Spec {
        id: "h5-increment-text-empty-inserted",
        before_name: "h5-increment-text-empty-before.accdb",
        before_sha: "12946f4ebe11d2bae07ac9d9fa88633df99626b243097169814cae050a43542e",
        after_sha: "f3f6999f7bbf36bdef4d9cdb2dfbf0e39853574d4c7e35262599b5e8321771b9",
        family: Family::Text,
        state: "empty",
        definition: 7,
        data: 8,
        root: 9,
        generated: 1,
        generated_position: 0,
        old_last: 0,
        old_live: 0,
        old_slots: 0,
        new_start: 4073,
        record_bytes: 23,
        history: 1,
    },
    Spec {
        id: "h5-increment-text-high-inserted",
        before_name: "h5-increment-text-high-before.accdb",
        before_sha: "52b642009b933cd05ca9ac128e3be71d9eb6b5c0aec1f12055881264f95afda6",
        after_sha: "3f66f4a6a62c67c4901440c143f3acab2712112b2cc981d00c8e627e8bc2e12b",
        family: Family::Text,
        state: "high",
        definition: 7,
        data: 8,
        root: 9,
        generated: 138,
        generated_position: 0,
        old_last: 137,
        old_live: 1,
        old_slots: 1,
        new_start: 4050,
        record_bytes: 23,
        history: 2,
    },
    Spec {
        id: "h5-increment-text-companion-null-high-inserted",
        before_name: "h5-increment-text-companion-null-high-before.accdb",
        before_sha: "4eba2cfc7010d814ff0680383aa883b53c8434739f962b1c5844564f6a167c7d",
        after_sha: "33c3b96dc0ef7453d925711ace94adaea4367122793c3ba402cb3ce0bd19c3b6",
        family: Family::Text,
        state: "companion-null-high",
        definition: 7,
        data: 8,
        root: 9,
        generated: 138,
        generated_position: 0,
        old_last: 137,
        old_live: 1,
        old_slots: 1,
        new_start: 4056,
        record_bytes: 17,
        history: 2,
    },
    Spec {
        id: "h5-increment-binary-empty-inserted",
        before_name: "h5-increment-binary-empty-before.accdb",
        before_sha: "b5eaf9b7ff845f4ae758e5372f5ec41158058b955860947afa2d2ea06f699337",
        after_sha: "ae02fbf489f1cbe356a907c5bf54dca95e09c6492b387bd53d102f4e5b8b8581",
        family: Family::Binary,
        state: "empty",
        definition: 7,
        data: 8,
        root: 9,
        generated: 1,
        generated_position: 0,
        old_last: 0,
        old_live: 0,
        old_slots: 0,
        new_start: 4075,
        record_bytes: 21,
        history: 1,
    },
    Spec {
        id: "h5-increment-binary-high-inserted",
        before_name: "h5-increment-binary-high-before.accdb",
        before_sha: "51383ed1ef58343fc0c0cd2040ca1e605d760edda5834cd22b03fd51f5918771",
        after_sha: "fa121624d63cef545f33c4fda2a3fabd38f76b026ae3bb9e67eb6b78f0218f35",
        family: Family::Binary,
        state: "high",
        definition: 7,
        data: 8,
        root: 9,
        generated: 138,
        generated_position: 0,
        old_last: 137,
        old_live: 1,
        old_slots: 1,
        new_start: 4055,
        record_bytes: 21,
        history: 2,
    },
    Spec {
        id: "h5-increment-binary-companion-null-high-inserted",
        before_name: "h5-increment-binary-companion-null-high-before.accdb",
        before_sha: "f97938689dbdeb607838db80472257e84ba0ee610e2c86d78bc04499a536fc56",
        after_sha: "b515102fabad208708d44c4a7400f78a235c8f465cddf520274a27ad7d411315",
        family: Family::Binary,
        state: "companion-null-high",
        definition: 7,
        data: 8,
        root: 9,
        generated: 138,
        generated_position: 0,
        old_last: 137,
        old_live: 1,
        old_slots: 1,
        new_start: 4059,
        record_bytes: 17,
        history: 2,
    },
    Spec {
        id: "h5-increment-fixed-empty-inserted",
        before_name: "h5-increment-fixed-empty-before.accdb",
        before_sha: "3a3c81a274c1ee4cc075592626210bc4ccb7ed00473bc896567e22b551b07bcd",
        after_sha: "b5f1897541d0b79d00f2e94e7efc1eacb80ba001d0ec9d88bea9361f9a4b1a3d",
        family: Family::Fixed,
        state: "empty",
        definition: 7,
        data: 8,
        root: 9,
        generated: 1,
        generated_position: 0,
        old_last: 0,
        old_live: 0,
        old_slots: 0,
        new_start: 4081,
        record_bytes: 15,
        history: 1,
    },
    Spec {
        id: "h5-increment-fixed-high-inserted",
        before_name: "h5-increment-fixed-high-before.accdb",
        before_sha: "7e782632784bc1e1c0a2ba40dbf1750bd2d546c14d638f5a407e122830fbd822",
        after_sha: "fb9418d3cb67a450c4dff74e29a28f51a52bc7e010a7926fded14ffe9a3363bd",
        family: Family::Fixed,
        state: "high",
        definition: 7,
        data: 8,
        root: 9,
        generated: 138,
        generated_position: 0,
        old_last: 137,
        old_live: 1,
        old_slots: 1,
        new_start: 4066,
        record_bytes: 15,
        history: 2,
    },
    Spec {
        id: "h5-increment-fixed-companion-null-high-inserted",
        before_name: "h5-increment-fixed-companion-null-high-before.accdb",
        before_sha: "dbb7736298c4436df06ceb6f20579d61ebc930567d051efd32627dc6822cb43f",
        after_sha: "bfbcb8c1a32e9f99288ed68b28cc34ac3c86c538a67f788df625c0cf85ea4b60",
        family: Family::Fixed,
        state: "companion-null-high",
        definition: 7,
        data: 8,
        root: 9,
        generated: 138,
        generated_position: 0,
        old_last: 137,
        old_live: 1,
        old_slots: 1,
        new_start: 4066,
        record_bytes: 15,
        history: 2,
    },
    Spec {
        id: "h6-increment-text-empty-inserted",
        before_name: "h6-increment-text-empty-before.accdb",
        before_sha: "086f65f0bfad8f86543cb1acf3292249933c6067ae2d2b047b3b4fdc2118a6c9",
        after_sha: "93e7c844658783de0a88dacd01be74330e70dd535b4b400f19fb6f4f3901573b",
        family: Family::Text,
        state: "empty",
        definition: 7,
        data: 8,
        root: 9,
        generated: 1,
        generated_position: 0,
        old_last: 0,
        old_live: 0,
        old_slots: 0,
        new_start: 4073,
        record_bytes: 23,
        history: 1,
    },
    Spec {
        id: "h6-increment-text-high-inserted",
        before_name: "h6-increment-text-high-before.accdb",
        before_sha: "1f93544e1bbff7a433ad3b141c10d50106fa29454119f6d7fb358a4a4522940e",
        after_sha: "5795352153ffb00efe6792df00a06546bde22b454254fa8b01522d0c9e6c04d1",
        family: Family::Text,
        state: "high",
        definition: 7,
        data: 8,
        root: 9,
        generated: 138,
        generated_position: 0,
        old_last: 137,
        old_live: 1,
        old_slots: 1,
        new_start: 4050,
        record_bytes: 23,
        history: 2,
    },
    Spec {
        id: "h6-increment-text-companion-null-high-inserted",
        before_name: "h6-increment-text-companion-null-high-before.accdb",
        before_sha: "f95a3d83dc1320059916525e97f620065b3be16dfbb49961b7db17de3200c763",
        after_sha: "ea7c41a157867670f4feb95fa474ff986bf024b0fb76c9f799fd13bd61b06be3",
        family: Family::Text,
        state: "companion-null-high",
        definition: 7,
        data: 8,
        root: 9,
        generated: 138,
        generated_position: 0,
        old_last: 137,
        old_live: 1,
        old_slots: 1,
        new_start: 4056,
        record_bytes: 17,
        history: 2,
    },
    Spec {
        id: "h6-increment-binary-empty-inserted",
        before_name: "h6-increment-binary-empty-before.accdb",
        before_sha: "fec61b9705ed92ace58e849fc9fcd9efa9f4ec182e94799faab99dca8784350c",
        after_sha: "0f713399ea1bf7301d47d1818d9bcfdb900c5e178706bc3b15c6f95a3db3c5a1",
        family: Family::Binary,
        state: "empty",
        definition: 7,
        data: 8,
        root: 9,
        generated: 1,
        generated_position: 0,
        old_last: 0,
        old_live: 0,
        old_slots: 0,
        new_start: 4075,
        record_bytes: 21,
        history: 1,
    },
    Spec {
        id: "h6-increment-binary-high-inserted",
        before_name: "h6-increment-binary-high-before.accdb",
        before_sha: "e81bf76e6b4e65dea20c7c13d8fff0d8c0b2c5e2b866eba3a3764bb0729a176f",
        after_sha: "bdc1d665e65956a623dce27b3a3c788f11d89515551be430df7e3ee5c0a9275b",
        family: Family::Binary,
        state: "high",
        definition: 7,
        data: 8,
        root: 9,
        generated: 138,
        generated_position: 0,
        old_last: 137,
        old_live: 1,
        old_slots: 1,
        new_start: 4055,
        record_bytes: 21,
        history: 2,
    },
    Spec {
        id: "h6-increment-binary-companion-null-high-inserted",
        before_name: "h6-increment-binary-companion-null-high-before.accdb",
        before_sha: "e3b034d4015561ca9c195253691fd3687402da6501eeca8916db97cf0e946500",
        after_sha: "c6d9c1c82339a8641bf8df3d918cbbcdd8c08a9acfa73ce1c5c2177a6b11da66",
        family: Family::Binary,
        state: "companion-null-high",
        definition: 7,
        data: 8,
        root: 9,
        generated: 138,
        generated_position: 0,
        old_last: 137,
        old_live: 1,
        old_slots: 1,
        new_start: 4059,
        record_bytes: 17,
        history: 2,
    },
    Spec {
        id: "h6-increment-fixed-empty-inserted",
        before_name: "h6-increment-fixed-empty-before.accdb",
        before_sha: "3fd3ca123c47a70a56e1111ef571227f8346c1896aa43d8a7b92d9e0a3ad456d",
        after_sha: "c1bcc5300ea593bc1d817cf96a0b94edd8d05225c8750e1c7477d381013d22b8",
        family: Family::Fixed,
        state: "empty",
        definition: 7,
        data: 8,
        root: 9,
        generated: 1,
        generated_position: 0,
        old_last: 0,
        old_live: 0,
        old_slots: 0,
        new_start: 4081,
        record_bytes: 15,
        history: 1,
    },
    Spec {
        id: "h6-increment-fixed-high-inserted",
        before_name: "h6-increment-fixed-high-before.accdb",
        before_sha: "a6e5065bb9f7c8eefc73bb6eff28278e2f053780b2b3b8a32b6860c6d49f774f",
        after_sha: "1655364783c44e2315e9a00bac7aa568799fc5dbb47bfd8d91844ef82e735f01",
        family: Family::Fixed,
        state: "high",
        definition: 7,
        data: 8,
        root: 9,
        generated: 138,
        generated_position: 0,
        old_last: 137,
        old_live: 1,
        old_slots: 1,
        new_start: 4066,
        record_bytes: 15,
        history: 2,
    },
    Spec {
        id: "h6-increment-fixed-companion-null-high-inserted",
        before_name: "h6-increment-fixed-companion-null-high-before.accdb",
        before_sha: "5083427c8bd09fb9e57978c83f512a457ebdf40efbc19e4e7dc1d71965f3bb39",
        after_sha: "3a1a23bf858337cc43bc257aae2a47d2fd43aa77d25f8872ce0020ef2eb237d9",
        family: Family::Fixed,
        state: "companion-null-high",
        definition: 7,
        data: 8,
        root: 9,
        generated: 138,
        generated_position: 0,
        old_last: 137,
        old_live: 1,
        old_slots: 1,
        new_start: 4066,
        record_bytes: 15,
        history: 2,
    },
    Spec {
        id: "h5-numbered-middle-inserted",
        before_name: "h5-default-original.accdb",
        before_sha: "c6dc5d36f35335ea2d9cafbc5cf6500dc135ea41a4d3f60d9241ac9e55714d12",
        after_sha: "c91b5f202dd1380a06b516e76fb37690b62a36748376f9b38fc9ac529e80cb5e",
        family: Family::Middle,
        state: "deleted-high",
        definition: 94,
        data: 97,
        root: 96,
        generated: 4,
        generated_position: 1,
        old_last: 3,
        old_live: 2,
        old_slots: 3,
        new_start: 4036,
        record_bytes: 15,
        history: 4,
    },
    Spec {
        id: "h6-numbered-middle-inserted",
        before_name: "h6-default-original.accdb",
        before_sha: "c6aae0208a2dfbd0b77f8867f49700718880ee761c7881df6ec5789843a8b456",
        after_sha: "3109cd8afe8a56db5f52c5c6c3f22a0f897fd9cbe7dc8142f260eb78e2238d03",
        family: Family::Middle,
        state: "deleted-high",
        definition: 111,
        data: 115,
        root: 113,
        generated: 4,
        generated_position: 1,
        old_last: 3,
        old_live: 2,
        old_slots: 3,
        new_start: 4036,
        record_bytes: 15,
        history: 4,
    },
];

pub struct Case {
    pub id: &'static str,
    pub table: &'static str,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub family: Family,
    pub state: &'static str,
    pub definition: u32,
    pub data: u32,
    pub root: u32,
    pub generated: i32,
    pub generated_position: usize,
    pub old_last: i32,
    pub old_live: usize,
    pub old_slots: usize,
    pub new_start: usize,
    pub record_bytes: usize,
    pub history: u32,
}

impl Case {
    /// None is original omission; Some(Null) is an explicitly supplied NULL.
    pub fn presence(&self) -> Vec<Option<Value>> {
        let inactive = self.state == "companion-null-high";
        match self.family {
            Family::Text => vec![
                None,
                Some(if inactive {
                    Value::Null
                } else {
                    Value::Text("new".into())
                }),
                Some(Value::Int32(17)),
            ],
            Family::Binary => vec![
                None,
                Some(if inactive {
                    Value::Null
                } else {
                    Value::Binary(vec![0x11, 0x22, 0x33, 0x44])
                }),
                Some(Value::Int32(17)),
            ],
            Family::Fixed => vec![
                None,
                Some(Value::Boolean(false)),
                Some(if inactive {
                    Value::Null
                } else {
                    Value::Int64(-7)
                }),
            ],
            Family::Middle => vec![Some(Value::Int32(103)), None, Some(Value::Int32(17))],
        }
    }

    pub fn rows(&self, inserted: bool) -> Vec<Vec<Value>> {
        let inactive = self.state == "companion-null-high";
        let mut rows = if self.family == Family::Middle {
            vec![
                vec![Value::Int32(101), Value::Int32(1), Value::Int32(10)],
                vec![Value::Int32(102), Value::Int32(2), Value::Int32(20)],
            ]
        } else if self.state == "empty" {
            Vec::new()
        } else {
            match self.family {
                Family::Text => vec![vec![
                    Value::Int32(1),
                    if inactive {
                        Value::Null
                    } else {
                        Value::Text("old".into())
                    },
                    Value::Int32(10),
                ]],
                Family::Binary => vec![vec![
                    Value::Int32(1),
                    if inactive {
                        Value::Null
                    } else {
                        Value::Binary(vec![0xc7, 0, 0xff])
                    },
                    Value::Int32(10),
                ]],
                Family::Fixed => vec![vec![
                    Value::Int32(1),
                    Value::Boolean(true),
                    if inactive {
                        Value::Null
                    } else {
                        Value::Int64(i64::MIN)
                    },
                ]],
                Family::Middle => unreachable!(),
            }
        };
        if inserted {
            rows.push(
                self.presence()
                    .into_iter()
                    .map(|value| value.unwrap_or(Value::Int32(self.generated)))
                    .collect(),
            );
        }
        rows
    }

    pub fn indexes(&self) -> &'static [&'static str] {
        if self.family == Family::Middle {
            &["PrimaryID", "PrimaryAlias"]
        } else {
            &["PrimaryId"]
        }
    }

    pub fn sql(&self) -> Option<&'static str> {
        match (self.family, self.state) {
            (Family::Text, "companion-null-high") => {
                Some("INSERT INTO Items (Label, Score) VALUES (NULL, 17)")
            }
            (Family::Text, _) => Some("INSERT INTO Items (Label, Score) VALUES ('new', 17)"),
            (Family::Binary, "companion-null-high") => {
                Some("INSERT INTO Items (Bytes, Score) VALUES (NULL, 17)")
            }
            (Family::Fixed, "companion-null-high") => {
                Some("INSERT INTO Items (Flag, Amount) VALUES (False, NULL)")
            }
            (Family::Middle, _) => Some("INSERT INTO Numbered (Earlier, Score) VALUES (103, 17)"),
            _ => None,
        }
    }

    pub fn columns(&self) -> [&'static str; 3] {
        match self.family {
            Family::Text => ["Id", "Label", "Score"],
            Family::Binary => ["Id", "Bytes", "Score"],
            Family::Fixed => ["Id", "Flag", "Amount"],
            Family::Middle => ["Earlier", "Id", "Score"],
        }
    }

    /// Literal CLI TSV policy for this finite source family, independent of
    /// the CLI's result serializer. SQL eligibility remains a separate method.
    pub fn readback(&self) -> String {
        let mut result = self.columns().join("\t");
        result.push('\n');

        for row in self.rows(true) {
            let cells: Vec<String> = row
                .into_iter()
                .map(|value| match value {
                    Value::Null => "NULL".into(),
                    Value::Boolean(value) => if value { "TRUE" } else { "FALSE" }.into(),
                    Value::Int32(value) => value.to_string(),
                    Value::Int64(value) => value.to_string(),
                    Value::Text(value) => format!("\"\"\"{value}\"\"\""),
                    Value::Binary(value) => {
                        let mut cell = String::from("0x");
                        for byte in value {
                            cell.push_str(&format!("{byte:02x}"));
                        }
                        cell
                    }
                    _ => panic!("closed original companion readback carriers"),
                })
                .collect();
            result.push_str(&cells.join("\t"));
            result.push('\n');
        }

        result
    }

    pub fn protected_offsets(&self) -> Vec<usize> {
        let start = self.data as usize * 4096 + self.new_start;
        let mut offsets = vec![
            0,
            self.definition as usize * 4096 + 20,
            self.definition as usize * 4096 + 67,
            start + 2 + 4 * self.generated_position,
            start + self.record_bytes - 1,
            self.root as usize * 4096 + 480,
            self.root as usize * 4096 + 4095,
            (if self.family == Family::Middle {
                self.definition + 1
            } else {
                3
            }) as usize
                * 4096
                + 4095,
        ];
        if self.state == "companion-null-high" {
            let old_bytes = match self.family {
                Family::Text => 23,
                Family::Binary => 20,
                Family::Fixed => 15,
                Family::Middle => unreachable!(),
            };
            let field = if self.family == Family::Fixed { 6 } else { 10 };
            offsets.push(self.data as usize * 4096 + 4096 - old_bytes + field);
        }
        if self.family == Family::Middle {
            offsets.push(self.data as usize * 4096 + 4053);
        }
        offsets
    }
}

pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let companions =
        PathBuf::from(env::var_os("TABLECRAFT_INCREMENT_COMPANION_SOURCE_DIR").unwrap());
    let genuine =
        PathBuf::from(env::var_os("TABLECRAFT_INCREMENT_COMPANION_ORIGINAL_DIR").unwrap());
    let afters =
        PathBuf::from(env::var_os("TABLECRAFT_INCREMENT_COMPANION_AFTER_SOURCE_DIR").unwrap());
    assert!(companions.is_absolute() && genuine.is_absolute() && afters.is_absolute());
    let mut cases = Vec::new();
    for spec in SPECS {
        let directory = if spec.family == Family::Middle {
            &genuine
        } else {
            &companions
        };
        let before = fs::read(directory.join(spec.before_name)).unwrap();
        verify(&before, spec.before_sha);
        let after = fs::read(afters.join(format!("{}.accdb", spec.id))).unwrap();
        verify(&after, spec.after_sha);
        assert_eq!(before.len(), after.len());
        cases.push(Case {
            id: spec.id,
            table: if spec.family == Family::Middle {
                "Numbered"
            } else {
                "Items"
            },
            before,
            after,
            family: spec.family,
            state: spec.state,
            definition: spec.definition,
            data: spec.data,
            root: spec.root,
            generated: spec.generated,
            generated_position: spec.generated_position,
            old_last: spec.old_last,
            old_live: spec.old_live,
            old_slots: spec.old_slots,
            new_start: spec.new_start,
            record_bytes: spec.record_bytes,
            history: spec.history,
        });
    }
    assert_eq!(cases.len(), 20);
    cases
}
