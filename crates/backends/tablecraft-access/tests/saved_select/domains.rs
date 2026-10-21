use super::*;
use tablecraft_access::NativeSource;
use tablecraft_core::values::Value;
use tablecraft_query::{ExecutionOptions, prepare};

#[test]
fn stored_domain_arguments_preserve_omission_and_reusable_explicit_parameters() {
    let mut parameter = row(2);
    parameter.name1 = Some("Criteria");
    parameter.flag = Some(10);
    let mut count = column("DCount('*','Items',)");
    count.name1 = Some("N");
    let mut sum = column("DSum('Score','Items',[Criteria])");
    sum.name1 = Some("S");
    sum.order = Some(&[0, 0, 0, 2]);
    let original = database_with_query_flags(&[parameter, count, sum], 0x80000000);
    for code in [1, 2, 5, 6] {
        let mut bytes = original.clone();
        bytes[20] = code;
        if code == 1 {
            bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
        }
        let source_bytes = bytes.clone();
        let mut source = NativeSource::new(
            Cursor::new(bytes.as_slice()),
            CatalogScope::new(7),
            budget(),
            &|| false,
        )
        .unwrap();
        let saved = source
            .read_saved_select("Saved", ParseLimits::default())
            .unwrap();
        let plan = prepare(saved.query(), &source).unwrap();
        for (criteria, sum) in [("Id>1", 50), ("Id=1", 10), ("Id>9", 0)] {
            let result = plan
                .execute(
                    &source,
                    &[("Criteria", Value::Text(criteria.into()))],
                    ExecutionOptions::default(),
                )
                .unwrap();
            let expected = if sum == 0 {
                Value::Null
            } else {
                Value::Int64(sum)
            };
            assert_eq!(result.rows(), [vec![Value::Int64(3), expected]]);
        }
        // The reader borrows an immutable byte slice; no writable storage is supplied.
        drop(source);
        assert_eq!(bytes, source_bytes);
    }
}
