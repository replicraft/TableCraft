//! Move owned runtime cell facts into the renderer-neutral presentation model.
use tablecraft_runtime::{
    Cell as RuntimeCell, CellKind as RuntimeKind, CollectionKind as RuntimeCollectionKind,
    ContentsTarget as RuntimeContentsTarget, DisplayControl as RuntimeDisplayControl,
};
use tablecraft_ui::{Cell, CellKind, CollectionKind, ContentsTarget, DisplayControl};

pub(super) fn rows(rows: Vec<Vec<RuntimeCell>>) -> Vec<Vec<Cell>> {
    rows.into_iter()
        .map(|row| row.into_iter().map(cell).collect())
        .collect()
}

pub(super) fn contents_target(target: &RuntimeContentsTarget) -> ContentsTarget {
    ContentsTarget {
        document_revision: target.document_revision,
        table: target.table.clone(),
        column: target.column.clone(),
        parent_ordinal: target.parent_ordinal,
    }
}

pub(super) fn collection_kind(kind: RuntimeCollectionKind) -> CollectionKind {
    match kind {
        RuntimeCollectionKind::Multivalue => CollectionKind::Multivalue,
        RuntimeCollectionKind::Attachments => CollectionKind::Attachments,
        RuntimeCollectionKind::VersionHistory => CollectionKind::VersionHistory,
    }
}

fn cell(cell: RuntimeCell) -> Cell {
    let kind = match cell.kind {
        RuntimeKind::Scalar {
            is_null,
            stored_calculation,
            boolean,
            display_control,
        } => CellKind::Scalar {
            is_null,
            stored_calculation,
            boolean,
            display_control: match display_control {
                RuntimeDisplayControl::Uninspected => DisplayControl::Uninspected,
                RuntimeDisplayControl::Absent => DisplayControl::Absent,
                RuntimeDisplayControl::Native(code) => DisplayControl::Native(code),
                RuntimeDisplayControl::Unsupported => DisplayControl::Unsupported,
            },
        },
        RuntimeKind::Multivalue { assigned, members } => CellKind::Multivalue { assigned, members },
        RuntimeKind::Attachments { assigned, members } => {
            CellKind::Attachments { assigned, members }
        }
        RuntimeKind::VersionHistory { assigned, members } => {
            CellKind::VersionHistory { assigned, members }
        }
    };
    Cell {
        text: cell.text,
        kind,
    }
}
