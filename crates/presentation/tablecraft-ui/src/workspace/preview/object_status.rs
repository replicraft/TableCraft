//! Unloaded objects exercise presentation states without a native read.
use super::{Preview, fixtures};
use crate::workspace::objects::{ObjectKind, ObjectTarget};

impl Preview {
    pub(super) fn prepare_unloaded_object(&mut self, loading: bool) {
        let open = self
            .state
            .prepare_object_open(ObjectTarget {
                document_revision: fixtures::REVISION,
                kind: ObjectKind::Table {
                    name: "Orders".into(),
                },
            })
            .expect("synthetic object can be opened without reading rows");
        assert!(open.needs_load);
        self.state.busy = loading;
        if !loading {
            self.state.error = Some(self.state.localizer.text("objects-mock-load-error"));
        }
    }
}
