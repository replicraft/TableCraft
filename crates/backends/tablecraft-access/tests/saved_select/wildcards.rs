use super::*;
use tablecraft_access::NativeSource;
use tablecraft_core::values::Value;
use tablecraft_query::{ExecutionOptions, prepare};

#[test]
fn stored_projection_wildcards_keep_qualifiers_comments_and_alias_refusals() {
    for (expression, expected, alias) in [
        ("N.*", "SELECT N.* FROM Items AS N", "N"),
        (
            "[Order Items].*",
            "SELECT [Order Items].* FROM Items AS [Order Items]",
            "Order Items",
        ),
        (
            " N . * -- stored comment",
            "SELECT N.* FROM Items AS N",
            "N",
        ),
        (" * ", "SELECT * FROM Items AS N", "N"),
    ] {
        let mut table = row(5);
        table.name1 = Some("Items");
        table.name2 = Some(alias);
        assert_eq!(
            compile(&[table, column(expression)]).unwrap().query(),
            parse_query(expected, Default::default()).unwrap().query(),
            "{expression}"
        );
        let mut aliased = column(expression);
        aliased.name1 = Some("Hidden");
        assert_eq!(compile(&[aliased]).err(), Some(E::InvalidMetadata));
    }

    for expression in [
        "N.*,1",
        "N.* FROM Hidden",
        "N.* INTO Hidden",
        "N.* AS Hidden",
        "N.*;",
        "N.*; SELECT 1",
        "N.* WHERE True",
        "N.* ORDER BY 1",
        "N.* UNION SELECT 1",
        "N.*) FROM Hidden --",
    ] {
        assert!(compile(&[column(expression)]).is_err(), "{expression}");
    }
}

#[test]
fn qualified_wildcard_executes_across_native_profiles_without_changing_bytes() {
    let mut table = row(5);
    table.name1 = Some("Items");
    table.name2 = Some("N");
    let original = database_with_query_flags(&[table, column("N.*")], 0x80000000);
    for code in [1, 2, 5, 6] {
        let mut bytes = original.clone();
        bytes[20] = code;
        if code == 1 {
            bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
        }
        let original = bytes.clone();
        let mut reader = Cursor::new(bytes);
        let mut source =
            NativeSource::new(&mut reader, CatalogScope::new(7), budget(), &|| false).unwrap();
        let saved = source
            .read_saved_select("Saved", ParseLimits::default())
            .unwrap();
        assert!(
            saved
                .definition()
                .rows()
                .iter()
                .any(|row| row.expression() == Some("N.*"))
        );
        let result = prepare(saved.query(), &source)
            .unwrap()
            .execute(&source, &[], ExecutionOptions::default())
            .unwrap();
        assert_eq!(result.columns(), ["Id", "Score"]);
        assert_eq!(
            result.rows(),
            [
                vec![Value::Int32(1), Value::Int32(10)],
                vec![Value::Int32(2), Value::Int32(20)],
                vec![Value::Int32(3), Value::Int32(30)],
            ]
        );
        drop(source);
        assert_eq!(reader.get_ref(), &original);
    }
}
