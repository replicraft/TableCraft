//! Grouped SELECT INTO uses the normal native decision/save/acknowledgment journey.
use super::creation_journey;
use tablecraft_runtime::SaveSchemaChange;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{fields, populated};

#[test]
fn actual_grouped_make_table_no_yes_keeps_prior_results_and_saves_complete_native_rows() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let before = populated::numbers(
        &seed,
        "Items",
        &[None, None, Some(1), Some(1), Some(2), Some(2), Some(2)],
    );
    let expected = populated::numbers(&before, "Archive", &[Some(2), Some(2), Some(3)]);
    creation_journey(
        before,
        expected,
        "SELECT CLng(Count(*)) AS CopiedScore INTO Archive FROM Items AS i GROUP BY i.CopiedScore HAVING Count(*)>=2 ORDER BY i.CopiedScore",
        "Archive",
        SaveSchemaChange::TableAdded { column_count: 1 },
        Some(3),
    );
}
