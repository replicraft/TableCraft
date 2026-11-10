//! App-private scratch-tab widgets shared by schema preservation acceptance.
use super::finish_reads;
use crate::workspace::Desktop;
use egui_kittest::Harness;
use tablecraft_ui::{ObjectKind, ObjectTab, ObjectTarget};

pub(in crate::workspace) const EXACT_SCRATCH: &str =
    " SELECT [Score], [Label] FROM [Table1]; -- keep raw scratch\n";

pub(in crate::workspace) fn scratch(
    harness: &mut Harness<'_, Desktop>,
    table: &ObjectTab,
) -> ObjectTab {
    let scratch = harness
        .state_mut()
        .state
        .prepare_object_open(ObjectTarget {
            document_revision: table.target.document_revision,
            kind: ObjectKind::Sql,
        })
        .unwrap()
        .tab;
    assert!(
        harness
            .state_mut()
            .state
            .replace_query_sql(EXACT_SCRATCH.into())
    );
    harness.state_mut().activate_object(table.clone());
    finish_reads(harness);
    scratch
}
