//! Complete synthetic text result for the resource-free Zoom presentation.
use super::Preview;
use crate::{Cell, Data, ObjectKind, ObjectTarget};

const NOTE_PARAGRAPHS: usize = 80;

impl Preview {
    pub(super) fn prepare_zoom_scene(&mut self) {
        let paragraph = "Customer correspondence — synthetic example\n\
            Keep the delivery address and contact preferences together.\n\
            This complete value remains available beyond the datasheet preview.\n\n";
        let mut text = paragraph.repeat(NOTE_PARAGRAPHS);
        text.push_str("End of the complete synthetic note.");
        let document_revision = self.state.database.as_ref().unwrap().document_revision;
        assert!(
            self.state
                .prepare_object_open(ObjectTarget {
                    document_revision,
                    kind: ObjectKind::Sql,
                })
                .is_some()
        );
        self.state.sql = "SELECT [Notes] FROM [Customers];".into();
        self.state.set_query_data(Data {
            row_ordinals: None,
            source_revision: None,
            table: "Query results".into(),
            offset: 0,
            columns: vec!["Notes".into()],
            rows: vec![vec![Cell::scalar(text)]],
            has_more: false,
        });
        assert!(self.state.open_zoom_cell());
    }
}
