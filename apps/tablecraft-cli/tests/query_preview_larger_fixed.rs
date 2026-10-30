//! Source-qualified fixed-purpose SQL engineering copies and retained refusals.
#[path = "../../../crates/engines/tablecraft-ace/tests/ace16_larger_fixed_images.rs"]
mod corpus;
#[path = "../../../crates/engines/tablecraft-ace/tests/ace16_long_value_updates_contract/model/source.rs"]
pub mod lval_source;

use std::io::Cursor;
use tablecraft_ace::{check_table, check_table_constraints, check_table_long_values, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;
use tablecraft_testkit::access::mutation_fixture as fixture;

#[test]
fn qualify_lval_refusal_source_before_larger_fixed_host() {
    let before = lval_source::source(6, 1);
    let mut reader = Cursor::new(&before);
    let mut budget = ResourceBudget::new(256 * 1024 * 1024, 100_000, 100_000_000);
    let scope = CatalogScope::new(1);

    let rows = read_table(&mut reader, scope, "Items", &mut budget, &|| false).unwrap();
    assert_eq!(
        rows.rows(),
        &[vec![
            Value::Int32(1),
            Value::Text("A".into()),
            Value::Binary(vec![7])
        ]]
    );
    check_table(&mut reader, scope, "Items", &mut budget, &|| false).unwrap();
    check_table_constraints(&mut reader, scope, "Items", &mut budget, &|| false).unwrap();
    check_table_long_values(&mut reader, scope, "Items", &mut budget, &|| false).unwrap();
}

#[path = "query_preview_larger_fixed/api.rs"]
mod api;
