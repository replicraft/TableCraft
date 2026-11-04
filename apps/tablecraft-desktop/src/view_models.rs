//! Move owned runtime cell facts into the renderer-neutral presentation model.
use tablecraft_runtime::{Cell as RuntimeCell, CellKind as RuntimeKind};
use tablecraft_ui::{Cell, CellKind};

pub(super) fn cell(cell: RuntimeCell) -> Cell {
    let kind = match cell.kind {
        RuntimeKind::Scalar {
            is_null,
            stored_calculation,
        } => CellKind::Scalar {
            is_null,
            stored_calculation,
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
