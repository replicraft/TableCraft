//! Source-free query recordset presentation; no runtime capability exists in this host.
use super::Preview;
use crate::{
    Cell, CellKind, Data, DisplayControl, HorizontalAlignment, QueryCollectionBinding,
    QueryContentsBindings, QueryContentsTarget,
};

const RESULT_ID: u64 = 941;
const HISTORY_RESULT_ID: u64 = 942;
const SQL: &str = "SELECT * FROM Items ORDER BY Id";

impl Preview {
    /// Show independently authored query collection facts in the resource-free preview.
    /// This issues no runtime result, native source identity, read or write capability.
    /// Returns false when the current preview cannot accept a SQL result publication.
    pub fn prepare_query_recordsets(&mut self) -> bool {
        self.prepare_recordsets(false)
    }

    /// Author stored-history presentation separately from the visible current Memo.
    /// This resource-free scene issues no runtime result or native capability.
    pub fn prepare_query_history_recordsets(&mut self) -> bool {
        self.prepare_recordsets(true)
    }

    fn prepare_recordsets(&mut self, history: bool) -> bool {
        if !self.state.replace_query_sql(SQL.into()) {
            return false;
        }
        let bindings = (0..3)
            .map(|row| QueryCollectionBinding {
                row,
                column: if history { 2 } else { 1 },
                slot: row,
            })
            .collect();
        let rows = [("1", 2), ("2", 1), ("3", 0)]
            .into_iter()
            .map(|(id, members)| {
                let summary = Cell {
                    presentation: None,
                    alignment: HorizontalAlignment::Left,
                    text: format!(
                        "{members} {}",
                        if history && members == 1 {
                            "stored version"
                        } else if history {
                            "stored versions"
                        } else if members == 1 {
                            "attachment"
                        } else {
                            "attachments"
                        }
                    ),
                    kind: if history {
                        CellKind::VersionHistory {
                            assigned: true,
                            members,
                        }
                    } else {
                        CellKind::Attachments {
                            assigned: true,
                            members,
                        }
                    },
                };
                if history {
                    vec![Cell::scalar(id), Cell::scalar("Current Memo text"), summary]
                } else {
                    vec![Cell::scalar(id), summary]
                }
            })
            .collect();
        self.state.set_query_data(
            Some(SQL.into()),
            Data {
                query_contents: QueryContentsBindings::new(
                    if history {
                        HISTORY_RESULT_ID
                    } else {
                        RESULT_ID
                    },
                    bindings,
                    0,
                ),
                source_revision: None,
                row_ordinals: None,
                table: "Query results".into(),
                offset: 0,
                columns: if history {
                    vec!["Id".into(), "CurrentMemo".into(), "StoredHistory".into()]
                } else {
                    vec!["Id".into(), "Score".into()]
                },
                rows,
                has_more: false,
            },
        )
    }

    pub(super) fn query_collection(&mut self, target: QueryContentsTarget) {
        if !matches!(target.result_id, RESULT_ID | HISTORY_RESULT_ID)
            || !self.state.can_view_query_contents(&target)
        {
            return;
        }
        let history = target.result_id == HISTORY_RESULT_ID;
        let rows = if history {
            match target.binding {
                0 => vec![
                    vec![
                        Cell::scalar("Earlier stored note"),
                        Cell::scalar("2026-10-01 09:00"),
                    ],
                    vec![Cell::scalar("Latest stored note"), null()],
                ],
                1 => vec![vec![Cell::scalar(""), null()]],
                2 => Vec::new(),
                _ => return,
            }
        } else {
            match target.binding {
                0 => vec![
                    vec![
                        Cell::scalar("report.pdf"),
                        Cell::scalar("application/pdf"),
                        uninspected(),
                    ],
                    vec![Cell::scalar(""), null(), uninspected()],
                ],
                1 => vec![vec![
                    Cell::scalar("notes.txt"),
                    Cell::scalar("text/plain"),
                    uninspected(),
                ]],
                2 => Vec::new(),
                _ => return,
            }
        };
        let request = self.request();
        self.state.begin_query_contents(target, request);
        self.state.set_query_contents(
            request,
            &target,
            Data {
                query_contents: None,
                source_revision: None,
                row_ordinals: None,
                table: if history {
                    "Stored history"
                } else {
                    "Attachments"
                }
                .into(),
                offset: 0,
                columns: if history {
                    vec!["Notes".into(), "Modified".into()]
                } else {
                    vec!["FileName".into(), "FileType".into(), "FileData".into()]
                },
                rows,
                has_more: false,
            },
        );
    }
}

fn null() -> Cell {
    Cell {
        presentation: None,
        alignment: HorizontalAlignment::Left,
        text: "NULL".into(),
        kind: CellKind::Scalar {
            is_null: true,
            stored_calculation: false,
            boolean: None,
            display_control: DisplayControl::Uninspected,
        },
    }
}

fn uninspected() -> Cell {
    Cell {
        presentation: None,
        alignment: HorizontalAlignment::Left,
        text: "Not inspected".into(),
        kind: CellKind::Uninspected,
    }
}
