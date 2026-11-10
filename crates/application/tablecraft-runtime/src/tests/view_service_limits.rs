//! Read services use actual table columns rather than fixed UI count ceilings.
use super::{request, stored::opened};
use crate::{
    Command, FilterComparison, FilterSelectionTarget, Response, TotalAggregate, TotalSpec,
    TotalsTarget,
};
use tablecraft_testkit::fixtures::access::mutation_fixture;

#[test]
fn filters_and_totals_cover_every_field_of_a_wider_observed_schema() {
    let bytes = mutation_fixture::wide::database(40);
    let (_directory, path, worker, revision) = opened(&bytes);
    let mut base = None;
    for index in 0..40 {
        let Response::FilteredData(publication) = request(
            &worker,
            Command::FilterSelection {
                target: FilterSelectionTarget {
                    document_revision: revision,
                    table: "Items".into(),
                    parent_ordinal: 0,
                    column: format!("Field{index}"),
                    filter_id: base
                        .as_ref()
                        .map(|handle: &crate::FilterHandle| handle.target().filter_id),
                    enabled: base.is_some(),
                },
                comparison: FilterComparison::Equal,
                base,
            },
        )
        .unwrap() else {
            panic!("expected complete filter publication");
        };
        assert_eq!(publication.filter_count, index + 1);
        assert_eq!(publication.source_rows, 1);
        assert_eq!(publication.matched_rows, 1);
        base = Some(publication.handle);
    }
    let Response::Totals(totals) = request(
        &worker,
        Command::Totals {
            target: TotalsTarget {
                document_revision: revision,
                table: "Items".into(),
                filter_id: base.as_ref().map(|handle| handle.target().filter_id),
                enabled: true,
            },
            specs: (0..40)
                .map(|index| TotalSpec {
                    column: format!("Field{index}"),
                    aggregate: TotalAggregate::Sum,
                })
                .collect(),
            base,
        },
    )
    .unwrap() else {
        panic!("expected complete totals publication");
    };
    assert_eq!(totals.cells.len(), 40);
    assert_eq!(totals.source_rows, 1);
    assert_eq!(totals.view_rows, 1);
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}
