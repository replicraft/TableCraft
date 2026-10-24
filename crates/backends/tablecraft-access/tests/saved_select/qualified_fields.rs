use super::*;

#[test]
fn registered_object_list_retains_qualified_connect_and_unresolved_vba_calls() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/examples/Access_Example_VBA.accdb");
    let original = std::fs::read(&path).unwrap();
    let mut reader = Cursor::new(&original);
    let definition = read_saved_query_definition(
        &mut reader,
        CatalogScope::new(1),
        "101_Object_List_qry",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        definition
            .rows()
            .iter()
            .any(|row| row.expression() == Some("msysobjects.Connect"))
    );

    let saved =
        reconstruct_saved_select(definition, Default::default(), &mut budget(), &|| false).unwrap();
    let expected = parse_query(
        "SELECT msysobjects.Name, GetObjectTypeName([Type]) AS TypeName, \
         msysobjects.Type, GetObjectDescription([Name]) AS Description, \
         msysobjects.DateUpdate, msysobjects.[Connect], msysobjects.DateCreate \
         FROM msysobjects \
         WHERE (((msysobjects.Type) Not In (2,3,8,-32757,-32758)) \
         AND ((msysobjects.Name) Not Like '~*' And (msysobjects.Name) Not Like 'Msys*')) \
         ORDER BY msysobjects.Type ASC, msysobjects.DateUpdate DESC, msysobjects.Name ASC",
        Default::default(),
    )
    .unwrap();
    assert_eq!(saved.query(), expected.query());
    assert!(
        saved
            .definition()
            .rows()
            .iter()
            .any(|row| row.expression() == Some("msysobjects.Connect"))
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn qualified_keyword_fields_do_not_admit_projection_or_expression_injection() {
    for expression in [
        "N.Connect FROM Hidden",
        "N.Connect AS Hidden",
        "N.Connect, N.Value",
        "N.Connect; SELECT 1",
        "N.Connect()",
        "N.Connect WHERE True",
        "N.Connect UNION SELECT 1",
    ] {
        assert!(compile(&[column(expression)]).is_err(), "{expression}");
    }
}
