//! Ordered original requests and independent image associations.
use super::raw::{self, PAGE, framing, geometry, guid, load_views, new_guid, values};
use crate::fixtures::access::original_fixed_wire::{self, Dialect, Group};
use std::sync::Arc;
use tablecraft_core::values::Value;

pub struct Case {
    pub header: u8,
    pub descending: bool,
    pub kind: usize,
    pub reverse: bool,
    pub before: Arc<[u8]>,
    pub after: Arc<[u8]>,
}

impl Case {
    pub fn updates(&self) -> Vec<(usize, Value, Value)> {
        let mut updates = match self.kind {
            1 => vec![(0, Value::Guid(guid(0)), Value::Guid(new_guid()))],
            2 => vec![(0, Value::Guid(guid(0)), Value::Guid(guid(1)))],
            3 => vec![
                (0, Value::Guid(guid(0)), Value::Guid(guid(1))),
                (1, Value::Guid(guid(1)), Value::Guid(guid(0))),
            ],
            4 => vec![(0, Value::Guid(guid(0)), Value::Null)],
            5 => vec![(157, Value::Null, Value::Guid(new_guid()))],
            6 => vec![(157, Value::Null, Value::Guid(guid(156)))],
            _ => panic!("outside the six frozen indexed GUID outcomes"),
        };
        if self.reverse {
            updates.reverse();
        }
        updates
    }

    pub fn changed_rows(&self) -> usize {
        if self.kind == 3 { 2 } else { 1 }
    }

    pub fn rows(&self, after: bool) -> Vec<Vec<Value>> {
        values(if after { self.kind } else { 0 })
    }

    pub fn wire(&self, binding: [u8; 32]) -> Vec<u8> {
        let updates = self.updates();
        original_fixed_wire::original_wire(
            Dialect::Guid,
            "Items",
            &[Group {
                column: "Group",
                updates: &updates,
            }],
            self.before.len(),
            binding,
        )
    }

    pub fn selected_record(&self) -> usize {
        let ordinal = self.updates()[0].0;
        let (_, data, _) = geometry(self.header);
        let (page, slot) = if ordinal < 140 {
            (data[0], ordinal)
        } else {
            (data[1], ordinal - 140)
        };
        page * PAGE + PAGE - raw::RECORD_BYTES * (slot + 1)
    }

    pub fn forgeries(&self) -> Vec<(usize, u8)> {
        let (definition, data, roots) = geometry(self.header);
        let record = self.selected_record();
        let mut offsets = vec![
            (0, 1),
            (definition * PAGE + 67, 1),
            (definition * PAGE + 79, 1),
            (data[0] * PAGE + 2, 1),
            (data[0] * PAGE + 14, 1),
            (record + 2, 1),
            (record + 22, 1),
            (record + 6, 1),
            (record + 10, 1),
            (record + 12, 1),
            (record + 21, 1),
            (record + 26, 2),
            (roots[0] * PAGE + 480, 1),
            (roots[1] * PAGE + 24, 1),
            (roots[1] * PAGE + 27, 1),
            (roots[1] * PAGE + 480, 1),
        ];
        // GLOBAL and both INDEX usage records are original physical roles.
        for reference in [
            256_u32,
            ((definition + 1) as u32) << 8 | 2,
            ((definition + 1) as u32) << 8 | 3,
        ] {
            let page = (reference >> 8) as usize;
            let slot = (reference & 255) as usize;
            let word = page * PAGE + 14 + 2 * slot;
            let start = usize::from(u16::from_le_bytes(
                self.before[word..word + 2].try_into().unwrap(),
            ));
            offsets.push((page * PAGE + start + 5, 1));
        }
        let (_, active, _) = framing(self.kind);
        if active < 3616 {
            offsets.push((roots[1] * PAGE + 480 + active, 1));
        }
        offsets
    }

    pub fn composite_after(&self) -> Vec<u8> {
        assert_eq!(self.kind, 1);
        let (_, data, _) = geometry(self.header);
        let at = data[0] * PAGE + PAGE - raw::RECORD_BYTES + 22;
        assert_eq!(&self.after[at..at + 4], &(-64_i32).to_le_bytes());
        let mut after = self.after.to_vec();
        after[at..at + 4].copy_from_slice(&(-63_i32).to_le_bytes());
        after
    }

    pub fn sql(&self) -> String {
        let updates = self.updates();
        if updates.len() == 2 {
            let first = literal(&updates.iter().find(|update| update.0 == 0).unwrap().2);
            let second = literal(&updates.iter().find(|update| update.0 == 1).unwrap().2);
            format!("UPDATE Items SET [Group]=IIf([ID]=1,{first},{second}) WHERE [ID] IN(1,2)")
        } else {
            let value = literal(&updates[0].2);
            let id = updates[0].0 + 1;
            format!("UPDATE Items SET [Group]={value} WHERE [ID]={id}")
        }
    }
}

pub fn display(value: &Value) -> String {
    match value {
        Value::Null => "NULL".to_owned(),
        Value::Guid(value) => {
            format!(
                "{{{}}}",
                uuid::Uuid::from_bytes(value.to_bytes()).hyphenated()
            )
        }
        _ => unreachable!(),
    }
}

fn literal(value: &Value) -> String {
    if matches!(value, Value::Null) {
        "NULL".to_owned()
    } else {
        format!("{{GUID {}}}", display(value))
    }
}

/// Return 28 distinct ordered requests over 24 unique independently authored AFTERs.
pub fn load_all(verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let views = load_views(verify);
    let mut cases = Vec::new();
    for view in views.iter().filter(|view| view.spec.kind != 0) {
        let original = views
            .iter()
            .find(|candidate| {
                candidate.spec.kind == 0
                    && candidate.spec.header == view.spec.header
                    && candidate.spec.descending == view.spec.descending
            })
            .unwrap();
        for reverse in if view.spec.kind == 3 {
            &[false, true][..]
        } else {
            &[false][..]
        } {
            cases.push(Case {
                header: view.spec.header,
                descending: view.spec.descending,
                kind: view.spec.kind,
                reverse: *reverse,
                before: original.raw.clone(),
                after: view.raw.clone(),
            });
        }
    }
    assert_eq!(cases.len(), 28);
    cases
}
