//! Host-owned cell facts; presentation never infers native storage or membership.

pub struct Cell {
    pub text: String,
    pub kind: CellKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CellKind {
    Scalar {
        is_null: bool,
        stored_calculation: bool,
    },
    Multivalue {
        assigned: bool,
        members: usize,
    },
    Attachments {
        assigned: bool,
        members: usize,
    },
    VersionHistory {
        assigned: bool,
        members: usize,
    },
}

impl Cell {
    /// Plain display text for fixtures or hosts without a stored NULL value.
    /// Native/query adapters must supply exact NULL and provenance flags instead.
    pub fn scalar(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: CellKind::Scalar {
                is_null: false,
                stored_calculation: false,
            },
        }
    }

    pub(crate) fn badge(&self) -> Option<&'static str> {
        match self.kind {
            CellKind::Scalar {
                is_null: true,
                stored_calculation: true,
            } => Some("stored"),
            CellKind::Scalar {
                stored_calculation: true,
                ..
            } => Some("stored"),
            CellKind::Scalar { is_null: true, .. } => Some("NULL"),
            CellKind::Scalar { .. } => None,
            CellKind::Multivalue { .. } => Some("list"),
            CellKind::Attachments { .. } => Some("encoded"),
            CellKind::VersionHistory { .. } => Some("history"),
        }
    }

    pub(crate) fn context(&self) -> Option<String> {
        match self.kind {
            CellKind::Scalar {
                is_null,
                stored_calculation,
            } => match (is_null, stored_calculation) {
                (false, false) => None,
                (true, false) => Some("NULL value".into()),
                (false, true) => {
                    Some("Stored calculated result; no formula evaluated; freshness unknown".into())
                }
                (true, true) => Some(
                    "Stored NULL calculated result; no formula evaluated; freshness unknown".into(),
                ),
            },
            CellKind::Multivalue { assigned, members } => {
                Some(membership("multivalue", assigned, members))
            }
            CellKind::Attachments { assigned, members } => Some(format!(
                "{}; payloads remain encoded and are not opened",
                membership("attachment", assigned, members),
            )),
            CellKind::VersionHistory { assigned, members } => Some(format!(
                "{}; stored order retained; current Memo text is separate",
                membership("stored version", assigned, members),
            )),
        }
    }
}

impl From<String> for Cell {
    fn from(text: String) -> Self {
        Self::scalar(text)
    }
}

impl From<&str> for Cell {
    fn from(text: &str) -> Self {
        Self::scalar(text)
    }
}

pub(crate) fn membership(kind: &str, assigned: bool, members: usize) -> String {
    if assigned {
        let unit = if members == 1 { "member" } else { "members" };
        format!(
            "Assigned {kind} collection; {members} complete {unit}. NULL payloads count as members"
        )
    } else {
        format!("No assigned {kind} collection")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_null_and_literal_null_text_have_distinct_provenance() {
        let plain = Cell::scalar("NULL");
        let stored = Cell {
            text: "NULL".into(),
            kind: CellKind::Scalar {
                is_null: true,
                stored_calculation: true,
            },
        };
        assert!(plain.badge().is_none());
        assert!(plain.context().is_none());
        assert_eq!(stored.badge(), Some("stored"));
        assert!(stored.context().unwrap().contains("freshness unknown"));
    }

    #[test]
    fn absent_empty_and_null_payload_membership_are_not_conflated() {
        let absent = Cell {
            text: "Unassigned".into(),
            kind: CellKind::Attachments {
                assigned: false,
                members: 0,
            },
        };
        let empty = Cell {
            text: "0 attachments".into(),
            kind: CellKind::Attachments {
                assigned: true,
                members: 0,
            },
        };
        let null_payload = Cell {
            text: "1 attachment".into(),
            kind: CellKind::Attachments {
                assigned: true,
                members: 1,
            },
        };
        assert!(absent.context().unwrap().starts_with("No assigned"));
        assert!(empty.context().unwrap().contains("0 complete members"));
        let context = null_payload.context().unwrap();
        assert!(context.contains("1 complete member"));
        assert!(context.contains("NULL payloads"));
        assert!(context.contains("not opened"));
    }
}
