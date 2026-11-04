//! Host-owned cell facts; presentation never infers native storage or membership.

pub struct Cell {
    pub text: String,
    pub kind: CellKind,
}

/// Original stored presentation metadata, without a guessed field default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayControl {
    Uninspected,
    Absent,
    Native(i16),
    Unsupported,
}

const CHECKBOX_DISPLAY_CONTROL: i16 = 106;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CellKind {
    Scalar {
        is_null: bool,
        stored_calculation: bool,
        boolean: Option<bool>,
        display_control: DisplayControl,
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
                boolean: None,
                display_control: DisplayControl::Uninspected,
            },
        }
    }

    pub(crate) fn badge(&self) -> Option<&'static str> {
        match self.kind {
            CellKind::Scalar {
                is_null: true,
                stored_calculation: true,
                ..
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

    pub(crate) fn checkbox_value(&self) -> Option<bool> {
        match self.kind {
            CellKind::Scalar {
                is_null: false,
                boolean,
                display_control: DisplayControl::Native(CHECKBOX_DISPLAY_CONTROL),
                ..
            } => boolean,
            _ => None,
        }
    }

    pub(crate) fn context(&self) -> Option<String> {
        let base = match self.kind {
            CellKind::Scalar {
                is_null,
                stored_calculation,
                ..
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
        };
        let presentation = match self.kind {
            CellKind::Scalar {
                boolean: Some(_),
                display_control,
                ..
            } => match display_control {
                DisplayControl::Uninspected => Some(
                    "Field display settings are not inspected; showing the stored value".to_owned(),
                ),
                DisplayControl::Absent => {
                    Some("No display control is declared; showing the stored value".to_owned())
                }
                DisplayControl::Unsupported => Some(
                    "Field display settings are unsupported; showing the stored value".to_owned(),
                ),
                DisplayControl::Native(CHECKBOX_DISPLAY_CONTROL) => None,
                DisplayControl::Native(109) => Some("This field uses a text box".to_owned()),
                DisplayControl::Native(111) => {
                    Some("Lookup display is not supported yet; showing the stored value".to_owned())
                }
                DisplayControl::Native(_) => Some(
                    "Field display settings are unsupported; showing the stored value".to_owned(),
                ),
            },
            _ => None,
        };
        match (base, presentation) {
            (Some(base), Some(presentation)) => Some(format!("{base}; {presentation}")),
            (base, presentation) => base.or(presentation),
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
                boolean: None,
                display_control: DisplayControl::Uninspected,
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

#[cfg(test)]
mod boolean_tests {
    use super::*;

    #[test]
    fn checkbox_requires_typed_non_null_boolean_and_explicit_native_checkbox_control() {
        for display_control in [
            DisplayControl::Uninspected,
            DisplayControl::Absent,
            DisplayControl::Unsupported,
            DisplayControl::Native(109),
            DisplayControl::Native(111),
            DisplayControl::Native(999),
            DisplayControl::Native(106),
        ] {
            let value = Cell {
                text: "true".into(),
                kind: CellKind::Scalar {
                    is_null: false,
                    stored_calculation: false,
                    boolean: Some(true),
                    display_control,
                },
            };
            assert_eq!(
                value.checkbox_value(),
                (display_control == DisplayControl::Native(106)).then_some(true)
            );
            if display_control != DisplayControl::Native(106) {
                let context = value.context().unwrap();
                assert!(!context.contains("DisplayControl"));
                assert!(!context.contains("Native"));
                assert!(!context.contains("109"));
                assert!(!context.contains("111"));
                assert!(!context.contains("999"));
                if display_control == DisplayControl::Native(109) {
                    assert_eq!(context, "This field uses a text box");
                } else {
                    assert!(context.ends_with("showing the stored value"));
                }
            }
        }
        for (is_null, boolean) in [(true, None), (true, Some(false)), (false, None)] {
            let value = Cell {
                text: "NULL".into(),
                kind: CellKind::Scalar {
                    is_null,
                    stored_calculation: false,
                    boolean,
                    display_control: DisplayControl::Native(106),
                },
            };
            assert_eq!(value.checkbox_value(), None);
        }
        assert_eq!(Cell::scalar("true").checkbox_value(), None);
    }
}
