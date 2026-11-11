//! Source-free query recordset presentation; no runtime capability exists in this host.
use super::Preview;
use crate::{
    Cell, CellKind, Data, DisplayControl, HorizontalAlignment, QueryCollectionBinding,
    QueryContentsBindings, QueryContentsTarget,
};

const RESULT_ID: u64 = 941;
const SQL: &str = "SELECT * FROM Items ORDER BY Id";

impl Preview {
    /// Show independently authored query collection facts in the resource-free preview.
    /// This issues no runtime result, native source identity, read or write capability.
    /// Returns false when the current preview cannot accept a SQL result publication.
    pub fn prepare_query_recordsets(&mut self) -> bool {
        if !self.state.replace_query_sql(SQL.into()) {
            return false;
        }
        let bindings = (0..3)
            .map(|row| QueryCollectionBinding {
                row,
                column: 1,
                slot: row,
            })
            .collect();
        let rows = [("1", 2), ("2", 1), ("3", 0)]
            .into_iter()
            .map(|(id, members)| {
                vec![
                    Cell::scalar(id),
                    Cell {
                        presentation: None,
                        alignment: HorizontalAlignment::Left,
                        text: format!(
                            "{members} {}",
                            if members == 1 {
                                "attachment"
                            } else {
                                "attachments"
                            }
                        ),
                        kind: CellKind::Attachments {
                            assigned: true,
                            members,
                        },
                    },
                ]
            })
            .collect();
        self.state.set_query_data(
            Some(SQL.into()),
            Data {
                query_contents: QueryContentsBindings::new(RESULT_ID, bindings, 0),
                source_revision: None,
                row_ordinals: None,
                table: "Query results".into(),
                offset: 0,
                columns: vec!["Id".into(), "Score".into()],
                rows,
                has_more: false,
            },
        )
    }

    pub(super) fn query_collection(&mut self, target: QueryContentsTarget) {
        if target.result_id != RESULT_ID || !self.state.can_view_query_contents(&target) {
            return;
        }
        let rows = match target.binding {
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
                table: "Attachments".into(),
                offset: 0,
                columns: vec!["FileName".into(), "FileType".into(), "FileData".into()],
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
