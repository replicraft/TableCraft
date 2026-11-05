//! Host-owned cell facts; presentation never infers native storage or membership.

use crate::{CollectionKind, FluentArgs, Localizer};

/// Default type-directed presentation; stored TextAlign overrides are uninspected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HorizontalAlignment {
    #[default]
    Left,
    Right,
}

pub struct Cell {
    /// Validated typed facts; never reconstructed from `text`.
    pub presentation: Option<crate::formatting::ScalarPresentation>,
    pub alignment: HorizontalAlignment,
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
            presentation: None,
            alignment: HorizontalAlignment::Left,
            text: text.into(),
            kind: CellKind::Scalar {
                is_null: false,
                stored_calculation: false,
                boolean: None,
                display_control: DisplayControl::Uninspected,
            },
        }
    }

    pub(crate) fn badge(&self, localizer: &Localizer) -> Option<String> {
        let key = match self.kind {
            CellKind::Scalar {
                stored_calculation: true,
                ..
            } => "datasheet-badge-stored",
            CellKind::Scalar { is_null: true, .. } => "datasheet-badge-null",
            CellKind::Scalar { .. } => return None,
            CellKind::Multivalue { .. } => "datasheet-badge-list",
            CellKind::Attachments { .. } => "datasheet-badge-encoded",
            CellKind::VersionHistory { .. } => "datasheet-badge-history",
        };
        Some(localizer.text(key))
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

    pub(crate) fn context(&self, localizer: &Localizer) -> Option<String> {
        let base = match self.kind {
            CellKind::Scalar {
                is_null,
                stored_calculation,
                ..
            } => {
                let key = match (is_null, stored_calculation) {
                    (false, false) => None,
                    (true, false) => Some("datasheet-null-context"),
                    (false, true) => Some("datasheet-stored-context"),
                    (true, true) => Some("datasheet-stored-null-context"),
                };
                key.map(|key| localizer.text(key))
            }
            CellKind::Multivalue { assigned, members } => Some(membership(
                localizer,
                CollectionKind::Multivalue,
                assigned,
                members,
            )),
            CellKind::Attachments { assigned, members } => {
                let mut arguments = FluentArgs::new();
                arguments.set(
                    "context",
                    membership(localizer, CollectionKind::Attachments, assigned, members),
                );
                Some(localizer.format("collections-encoded-context", &arguments))
            }
            CellKind::VersionHistory { assigned, members } => {
                let mut arguments = FluentArgs::new();
                arguments.set(
                    "context",
                    membership(localizer, CollectionKind::VersionHistory, assigned, members),
                );
                Some(localizer.format("collections-history-context", &arguments))
            }
        };

        let presentation = match self.kind {
            CellKind::Scalar {
                boolean: Some(_),
                display_control,
                ..
            } => {
                let key = match display_control {
                    DisplayControl::Uninspected => Some("datasheet-control-uninspected"),
                    DisplayControl::Absent => Some("datasheet-control-absent"),
                    DisplayControl::Unsupported => Some("datasheet-control-unsupported"),
                    DisplayControl::Native(CHECKBOX_DISPLAY_CONTROL) => None,
                    DisplayControl::Native(109) => Some("datasheet-control-textbox"),
                    DisplayControl::Native(111) => Some("datasheet-control-lookup"),
                    DisplayControl::Native(_) => Some("datasheet-control-unsupported"),
                };
                key.map(|key| localizer.text(key))
            }
            _ => None,
        };

        match (base, presentation) {
            (Some(base), Some(presentation)) => {
                let mut arguments = FluentArgs::new();
                arguments.set("base", base);
                arguments.set("presentation", presentation);
                Some(localizer.format("datasheet-context-pair", &arguments))
            }
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

pub(crate) fn membership(
    localizer: &Localizer,
    kind: CollectionKind,
    assigned: bool,
    members: usize,
) -> String {
    let kind_key = match kind {
        CollectionKind::Multivalue => "collections-multivalue",
        CollectionKind::Attachments => "collections-attachment",
        CollectionKind::VersionHistory => "collections-stored-version",
    };
    let key = if assigned {
        "collections-assigned"
    } else {
        "collections-unassigned"
    };
    let mut arguments = FluentArgs::new();
    arguments.set("kind", localizer.text(kind_key));
    arguments.set("count", members);
    localizer.format(key, &arguments)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_null_and_literal_null_text_have_distinct_provenance() {
        let plain = Cell::scalar("NULL");
        let stored = Cell {
            presentation: None,
            alignment: HorizontalAlignment::Left,
            text: "NULL".into(),
            kind: CellKind::Scalar {
                is_null: true,
                stored_calculation: true,
                boolean: None,
                display_control: DisplayControl::Uninspected,
            },
        };
        assert!(plain.badge(&Localizer::default()).is_none());
        assert!(plain.context(&Localizer::default()).is_none());
        assert_eq!(
            stored.badge(&Localizer::default()),
            Some("stored".to_owned())
        );
        assert!(
            stored
                .context(&Localizer::default())
                .unwrap()
                .contains("freshness unknown")
        );
    }

    #[test]
    fn absent_empty_and_null_payload_membership_are_not_conflated() {
        let absent = Cell {
            presentation: None,
            alignment: HorizontalAlignment::Left,
            text: "Unassigned".into(),
            kind: CellKind::Attachments {
                assigned: false,
                members: 0,
            },
        };
        let empty = Cell {
            presentation: None,
            alignment: HorizontalAlignment::Left,
            text: "0 attachments".into(),
            kind: CellKind::Attachments {
                assigned: true,
                members: 0,
            },
        };
        let null_payload = Cell {
            presentation: None,
            alignment: HorizontalAlignment::Left,
            text: "1 attachment".into(),
            kind: CellKind::Attachments {
                assigned: true,
                members: 1,
            },
        };
        assert!(
            absent
                .context(&Localizer::default())
                .unwrap()
                .replace(['\u{2068}', '\u{2069}'], "")
                .starts_with("No assigned")
        );
        // Ignore only Fluent direction-isolation marks for English prose
        // checks; count, membership, plurality and provenance remain distinct.
        let empty_context = empty
            .context(&Localizer::default())
            .unwrap()
            .replace(['\u{2068}', '\u{2069}'], "");
        assert!(empty_context.contains('0'));
        assert!(empty_context.contains("complete members"));
        let context = null_payload
            .context(&Localizer::default())
            .unwrap()
            .replace(['\u{2068}', '\u{2069}'], "");
        assert!(context.contains('1'));
        assert!(context.contains("complete member."));
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
                presentation: None,
                alignment: HorizontalAlignment::Left,
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
                let context = value.context(&Localizer::default()).unwrap();
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
                presentation: None,
                alignment: HorizontalAlignment::Left,
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
