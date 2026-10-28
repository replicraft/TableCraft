//! Explicit complete index requests preserve count, ownership and quota checks.
#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod base;
#[allow(dead_code)]
#[path = "composite_indexes/fixture.rs"]
mod composite_fixture;
#[path = "large_index_selections/fixture.rs"]
mod fixture;
use tablecraft_testkit::access as support;

#[path = "large_complete_index_contract/complete.rs"]
mod complete;
#[path = "large_complete_index_contract/guards.rs"]
mod guards;

use tablecraft_ace::{
    IndexProjectionSelection, IndexReadRequest, InspectError, NativeReadSource,
    TableSnapshotLimits, read_table_index_with_request,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(2 * 1024 * 1024, 1024, 1_000_000)
}

fn complete_request(maximum_rows: u32) -> IndexReadRequest<'static> {
    IndexReadRequest {
        columns: None,
        selection: IndexProjectionSelection::All,
        maximum_rows,
    }
}

fn complete<R: NativeReadSource>(
    source: &mut R,
) -> Result<tablecraft_ace::TableSnapshot, InspectError> {
    read_table_index_with_request(
        source,
        CatalogScope::new(1),
        "Items",
        "Compound",
        complete_request(3),
        &mut budget(),
        &|| false,
    )
}
