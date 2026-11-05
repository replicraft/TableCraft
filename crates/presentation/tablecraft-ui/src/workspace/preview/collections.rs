//! Synthetic collection lifecycle states use the existing source-bound UI contract.
use super::{Preview, fixtures};
use crate::{CellKind, CollectionKind, Data};

#[derive(Clone, Copy)]
pub(super) enum CollectionScene {
    Loading,
    Error,
    Unassigned,
    Empty,
    Null,
    AttachmentNull,
    HistoryNull,
}

impl Preview {
    pub(super) fn prepare_collection_scene(&mut self, scene: CollectionScene) {
        let (kind, column) = match scene {
            CollectionScene::Loading | CollectionScene::Error | CollectionScene::AttachmentNull => {
                (CollectionKind::Attachments, "Documents")
            }
            CollectionScene::HistoryNull => (CollectionKind::VersionHistory, "Notes"),
            CollectionScene::Unassigned | CollectionScene::Empty | CollectionScene::Null => {
                (CollectionKind::Multivalue, "Categories")
            }
        };
        let assigned = !matches!(scene, CollectionScene::Unassigned);
        let child = collection_data(scene, kind);
        let mut parent = fixtures::page("Customers", 0);
        let column_index = parent
            .column_index(column)
            .expect("the synthetic parent declares its collection field");
        let cell = &mut parent.rows[0][column_index];
        cell.kind = match kind {
            CollectionKind::Multivalue => CellKind::Multivalue {
                assigned,
                members: child.rows.len(),
            },
            CollectionKind::Attachments => CellKind::Attachments {
                assigned,
                members: child.rows.len(),
            },
            CollectionKind::VersionHistory => CellKind::VersionHistory {
                assigned,
                members: child.rows.len(),
            },
        };
        match scene {
            CollectionScene::Unassigned | CollectionScene::Empty => cell.text.clear(),
            CollectionScene::Null => {
                cell.text =
                    "NULL; NULL; Needs \"approval\"; follow-up; Needs \"approval\"; follow-up"
                        .into();
            }
            CollectionScene::AttachmentNull => cell.text = "1 encoded attachment".into(),
            CollectionScene::Loading | CollectionScene::Error | CollectionScene::HistoryNull => {}
        }
        let target = parent
            .contents_target(0, column_index)
            .expect("the synthetic collection target retains its parent source");
        self.state.set_table_data(parent);
        let parent = self
            .state
            .data
            .as_ref()
            .expect("published synthetic parent");
        self.state.table_selection.select(parent, 0, column_index);

        let request = self.request();
        self.state.begin_contents(target.clone(), request);
        match scene {
            CollectionScene::Loading => {}
            CollectionScene::Error => self.state.fail_contents(
                request,
                target,
                self.state.localizer.text("collections-mock-error"),
            ),
            _ => self
                .state
                .set_contents(request, target, kind, assigned, child),
        }
    }
}

fn collection_data(scene: CollectionScene, kind: CollectionKind) -> Data {
    let mut data = fixtures::contents(kind);
    match scene {
        CollectionScene::Unassigned | CollectionScene::Empty => data.rows.clear(),
        CollectionScene::Null => {
            data.rows = vec![
                vec![fixtures::null()],
                vec![fixtures::text("NULL")],
                vec![fixtures::text("Needs \"approval\"; follow-up")],
                vec![fixtures::text("Needs \"approval\"; follow-up")],
            ];
        }
        CollectionScene::AttachmentNull => {
            data.rows = vec![vec![
                fixtures::null(),
                fixtures::text("application/octet-stream"),
                fixtures::null(),
            ]];
        }
        CollectionScene::HistoryNull => data.rows[0][1] = fixtures::null(),
        CollectionScene::Loading | CollectionScene::Error => {}
    }
    data
}

#[cfg(test)]
mod tests;
