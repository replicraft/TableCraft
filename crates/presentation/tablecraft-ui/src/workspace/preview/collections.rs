//! Synthetic collection lifecycle states use the existing source-bound UI contract.
use super::{Preview, Scene, fixtures};
use crate::{CellKind, CollectionKind, ContentsTarget, Data};

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
    pub(super) fn collection(&mut self, target: ContentsTarget, kind: CollectionKind) {
        if !self.state.can_view_contents(&target) {
            return;
        }
        let Some(parent) = &self.state.data else {
            return;
        };
        let Some(column) = parent.column_index(&target.column) else {
            return;
        };
        let Some(row) = parent.row_index(target.parent_ordinal) else {
            return;
        };
        let Some(cell) = parent.rows.get(row).and_then(|row| row.get(column)) else {
            return;
        };
        let (assigned, members) = match cell.kind {
            CellKind::Multivalue { assigned, members }
            | CellKind::Attachments { assigned, members }
            | CellKind::VersionHistory { assigned, members } => (assigned, members),
            CellKind::Scalar { .. } => return,
        };
        let scene = collection_scene(self.scene).filter(|scene| {
            let (expected_kind, expected_column) = scene.source();
            target.table == "Customers"
                && target.parent_ordinal == 0
                && target.column == expected_column
                && kind == expected_kind
                && assigned == !matches!(scene, CollectionScene::Unassigned)
                && members == collection_data(*scene, kind).rows.len()
        });
        let mut child = scene.map_or_else(
            || fixtures::contents(kind),
            |scene| collection_data(scene, kind),
        );
        if members == 0 {
            child.rows.clear();
        }

        let request = self.request();
        self.state.begin_contents(target.clone(), request);
        match scene {
            Some(CollectionScene::Loading) => {}
            Some(CollectionScene::Error) => self.state.fail_contents(
                request,
                target,
                self.state.localizer.text("collections-mock-error"),
            ),
            _ => self
                .state
                .set_contents(request, target, kind, assigned, child),
        }
    }

    pub(super) fn prepare_collection_scene(&mut self, scene: CollectionScene) {
        let (kind, column) = scene.source();
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

impl CollectionScene {
    fn source(self) -> (CollectionKind, &'static str) {
        match self {
            Self::Loading | Self::Error | Self::AttachmentNull => {
                (CollectionKind::Attachments, "Documents")
            }
            Self::HistoryNull => (CollectionKind::VersionHistory, "Notes"),
            Self::Unassigned | Self::Empty | Self::Null => {
                (CollectionKind::Multivalue, "Categories")
            }
        }
    }
}

fn collection_scene(scene: Scene) -> Option<CollectionScene> {
    match scene {
        Scene::CollectionLoading => Some(CollectionScene::Loading),
        Scene::CollectionError => Some(CollectionScene::Error),
        Scene::CollectionUnassigned => Some(CollectionScene::Unassigned),
        Scene::CollectionEmpty => Some(CollectionScene::Empty),
        Scene::CollectionNull => Some(CollectionScene::Null),
        Scene::AttachmentNull => Some(CollectionScene::AttachmentNull),
        Scene::HistoryNull => Some(CollectionScene::HistoryNull),
        _ => None,
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
