use super::*;
use tablecraft_access::{NativeSource, reconstruct_saved_read};
use tablecraft_core::values::Value;
use tablecraft_query::{QueryError, prepare};

fn kind() -> Fragment<'static> {
    Fragment {
        flag: Some(6),
        ..row(1)
    }
}
fn role<'a>(expression: &'a str, flag: i16, order: &'static [u8]) -> Fragment<'a> {
    Fragment {
        flag: Some(flag),
        order: Some(order),
        ..column(expression)
    }
}
fn cross(pivot: &str) -> Vec<Fragment<'_>> {
    vec![
        kind(),
        Fragment {
            name1: Some("Items"),
            ..row(5)
        },
        Fragment {
            name1: Some("Bucket"),
            ..role("0", 2, &[0, 0, 0, 1])
        },
        Fragment {
            name1: Some("Total"),
            ..role("Count(*)", 2, &[0, 0, 0, 2])
        },
        Fragment {
            name1: Some("Measure"),
            ..role("Count(*)", 0, &[0, 0, 0, 3])
        },
        role(pivot, 1, &[0, 0, 0, 4]),
        Fragment {
            expression: Some("0"),
            flag: Some(2),
            ..row(9)
        },
        Fragment {
            expression: Some("[Id]"),
            flag: Some(1),
            order: Some(&[0, 0, 0, 2]),
            ..row(9)
        },
        Fragment {
            expression: Some("[Bucket]"),
            ..row(11)
        },
    ]
}
fn compile_read(rows: &[Fragment<'_>]) -> Result<tablecraft_access::SavedRead, E> {
    reconstruct_saved_read(
        definition(rows, 0x10),
        Default::default(),
        &mut budget(),
        &|| false,
    )
}
#[test]
fn crosstab_stored_brackets_apply_to_table_row_and_transform_aliases() {
    let mut rows = cross("Id IN (1,2,3)");
    rows[1].name1 = Some("[Items]");
    rows[2].name1 = Some("[Bucket]");
    rows[3].name1 = Some("[Total]");
    rows[4].name1 = Some("[Measure]");
    let expected=parse_query("TRANSFORM Count(*) AS Measure SELECT 0 AS Bucket,Count(*) AS Total FROM Items GROUP BY 0 ORDER BY Bucket PIVOT Id IN (1,2,3)",Default::default()).unwrap();
    assert_eq!(compile_read(&rows).unwrap().query(), expected.query());
}
#[test]
fn crosstab_retains_and_executes_a_stored_derived_source() {
    let mut rows = cross("Id IN (1,2,3)");
    rows[1].name1 = None;
    rows[1].expression = Some("SELECT Id FROM Items WHERE Id<3");
    let saved = compile_read(&rows).unwrap();
    let expected = parse_query("TRANSFORM Count(*) AS Measure SELECT 0 AS Bucket,Count(*) AS Total FROM (SELECT Id FROM Items WHERE Id<3) GROUP BY 0 ORDER BY Bucket PIVOT Id IN (1,2,3)", Default::default()).unwrap();
    assert_eq!(saved.query(), expected.query());
    let bytes = database_with_query_flags(&rows, 0x10);
    let before = bytes.clone();
    let source = NativeSource::new(
        Cursor::new(bytes.as_slice()),
        CatalogScope::new(7),
        budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        prepare(saved.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        &[vec![
            Value::Int64(0),
            Value::Int64(2),
            Value::Int64(1),
            Value::Int64(1),
            Value::Null
        ]]
    );
    drop(source);
    assert_eq!(bytes, before);
}
#[test]
fn crosstab_uses_the_same_nested_outer_join_forest() {
    use super::joins::{join, source};

    let mut rows = cross("c.Id IN (1,2,3)");
    rows[1] = source("Items", Some("a"));
    rows[7].expression = Some("c.Id");
    rows.extend([
        source("Items", Some("b")),
        source("Items", Some("c")),
        join(2, "a", "b", "a.Id=b.Id"),
        Fragment {
            order: Some(&[0, 0, 0, 2]),
            ..join(2, "b", "c", "b.Id=c.Id")
        },
        Fragment {
            order: Some(&[0, 0, 0, 3]),
            ..join(2, "b", "a", "b.Id<>2")
        },
    ]);
    let saved = compile_read(&rows).unwrap();
    let expected = parse_query("TRANSFORM Count(*) AS Measure SELECT 0 AS Bucket,Count(*) AS Total FROM (Items AS a LEFT JOIN Items AS b ON (b.Id<>2) AND (a.Id=b.Id)) LEFT JOIN Items AS c ON b.Id=c.Id GROUP BY 0 ORDER BY Bucket PIVOT c.Id IN (1,2,3)", Default::default()).unwrap();
    assert_eq!(saved.query(), expected.query());
    let bytes = database_with_query_flags(&rows, 0x10);
    let before = bytes.clone();
    let source = NativeSource::new(
        Cursor::new(bytes.as_slice()),
        CatalogScope::new(7),
        budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        prepare(saved.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        &[vec![
            Value::Int64(0),
            Value::Int64(3),
            Value::Int64(1),
            Value::Null,
            Value::Int64(1)
        ]]
    );
    drop(source);
    assert_eq!(bytes, before);
}
#[test]
fn crosstab_roles_fixed_headings_aliases_parameters_and_hidden_options_are_preserved() {
    let mut rows = cross("[Id] IN (3,1,4)");
    rows.push(Fragment {
        name1: Some("Floor"),
        flag: Some(4),
        ..row(2)
    });
    rows.push(Fragment {
        expression: Some("[Id] >= [Floor]"),
        ..row(8)
    });
    rows.push(Fragment {
        expression: Some("Count(*) > 1"),
        ..row(10)
    });
    let saved = compile_read(&rows).unwrap();
    let expected=parse_query("PARAMETERS Floor LONG; TRANSFORM Count(*) AS Measure SELECT 0 AS Bucket,Count(*) AS Total FROM Items WHERE Id>=Floor GROUP BY 0 HAVING Count(*)>1 ORDER BY Bucket PIVOT Id IN (3,1,4)",Default::default()).unwrap();
    assert_eq!(saved.query(), expected.query());
    assert_eq!(saved.definition().origin().flags(), 0x10);
    assert_eq!(saved.definition().rows()[7].flag(), Some(1));
    for (flag, name, sql) in [
        (
            16,
            Some("1"),
            "TRANSFORM Count(*) AS Measure SELECT TOP 1 0 AS Bucket,Count(*) AS Total FROM Items GROUP BY 0 ORDER BY Bucket PIVOT Id",
        ),
        (
            4,
            None,
            "TRANSFORM Count(*) AS Measure SELECT 0 AS Bucket,Count(*) AS Total FROM Items GROUP BY 0 ORDER BY Bucket PIVOT Id WITH OWNERACCESS OPTION",
        ),
    ] {
        let mut rows = cross("[Id]");
        rows.push(Fragment {
            flag: Some(flag),
            name1: name,
            ..row(3)
        });
        let saved = compile_read(&rows).unwrap();
        assert_eq!(
            saved.query(),
            parse_query(sql, Default::default()).unwrap().query()
        );
        let bytes = database_with_query_flags(&rows, 0x10);
        let source = NativeSource::new(Cursor::new(bytes), CatalogScope::new(7), budget(), &|| {
            false
        })
        .unwrap();
        let plan = prepare(saved.query(), &source);
        if flag == 16 {
            assert_eq!(
                plan.unwrap()
                    .execute(&source, &[], Default::default())
                    .unwrap()
                    .rows(),
                [vec![
                    Value::Int64(0),
                    Value::Int64(3),
                    Value::Int64(1),
                    Value::Int64(1),
                    Value::Int64(1)
                ]]
            );
        } else {
            assert_eq!(plan.err(), Some(QueryError::Unsupported));
        }
    }
    assert!(matches!(
        reconstruct_saved_select(
            definition(&cross("[Id]"), 0x10),
            Default::default(),
            &mut budget(),
            &|| false
        ),
        Err(E::Unsupported)
    ));
}
#[test]
fn explicit_saved_crosstabs_execute_dynamic_and_fixed_headings_on_all_current_layouts() {
    for code in [1, 2, 5, 6] {
        for (pivot, headings, values) in [
            (
                "[Id]",
                vec!["Bucket", "Total", "1", "2", "3"],
                vec![
                    Value::Int64(0),
                    Value::Int64(3),
                    Value::Int64(1),
                    Value::Int64(1),
                    Value::Int64(1),
                ],
            ),
            (
                "[Id] IN (3,1,4)",
                vec!["Bucket", "Total", "3", "1", "4"],
                vec![
                    Value::Int64(0),
                    Value::Int64(3),
                    Value::Int64(1),
                    Value::Int64(1),
                    Value::Null,
                ],
            ),
        ] {
            let mut bytes = database_with_query_flags(&cross(pivot), 0x10);
            bytes[20] = code;
            if code == 1 {
                bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
            }
            let before = bytes.clone();
            let mut source = NativeSource::new(
                Cursor::new(bytes.as_slice()),
                CatalogScope::new(7),
                budget(),
                &|| false,
            )
            .unwrap();
            let saved = source
                .read_saved_query("Saved", Default::default())
                .unwrap();
            let result = prepare(saved.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .unwrap();
            assert_eq!(result.columns(), headings);
            assert_eq!(result.rows(), std::slice::from_ref(&values));
            // Dynamic crosstabs are not implicitly scanned to bind a named schema.
            let named = parse_query("SELECT * FROM Saved", Default::default()).unwrap();
            if pivot.contains(" IN ") {
                assert_eq!(
                    prepare(named.query(), &source)
                        .unwrap()
                        .execute(&source, &[], Default::default())
                        .unwrap()
                        .rows(),
                    std::slice::from_ref(&values)
                );
            } else {
                assert_eq!(
                    prepare(named.query(), &source).err(),
                    Some(QueryError::Unsupported)
                );
            }
            drop(source);
            assert_eq!(bytes, before);
        }
    }
}
#[test]
fn crosstab_conflicting_roles_groups_flags_payloads_and_expression_injection_fail_explicitly() {
    for (index, flag) in [(0, 1), (2, 3), (4, -1), (5, 2), (6, 0), (7, 0)] {
        let mut rows = cross("[Id]");
        rows[index].flag = Some(flag);
        assert!(compile_read(&rows).is_err(), "{index} {flag}");
    }
    for index in [0, 4, 5] {
        let mut rows = cross("[Id]");
        rows.remove(index);
        assert!(compile_read(&rows).is_err(), "missing {index}");
    }
    for (index, flag) in [(5, 0), (4, 1)] {
        let mut rows = cross("[Id]");
        rows[index].flag = Some(flag);
        rows[index].name1 = None;
        assert!(matches!(compile_read(&rows), Err(E::InvalidMetadata)));
    }
    let mut rows = cross("[Id]");
    rows.retain(|row| !(row.attribute == Some(6) && row.flag == Some(2)));
    assert!(matches!(compile_read(&rows), Err(E::InvalidMetadata)));
    let mut rows = cross("[Id]");
    rows.push(Fragment {
        expression: Some("[Id]"),
        flag: Some(1),
        order: Some(&[0, 0, 0, 3]),
        ..row(9)
    });
    assert!(matches!(compile_read(&rows), Err(E::InvalidMetadata)));
    let mut rows = cross("[Id]");
    rows[7].expression = Some("Score");
    assert!(matches!(compile_read(&rows), Err(E::InvalidMetadata)));
    let mut rows = cross("[Id]");
    rows[5].name1 = Some("Ignored");
    assert!(compile_read(&rows).is_err());
    for (index, text) in [
        (2, "0; DELETE FROM Items"),
        (4, "Count(*); SELECT 1"),
        (5, "[Id] IN (1,2) ORDER BY 1"),
    ] {
        let mut rows = cross("[Id]");
        rows[index].expression = Some(text);
        assert!(compile_read(&rows).is_err());
    }
    let mut rows = cross("[Id]");
    rows[3].order = Some(&[0, 0, 0, 1]);
    assert_eq!(compile_read(&rows).unwrap().query(), parse_query("TRANSFORM Count(*) AS Measure SELECT 0 AS Bucket,Count(*) AS Total FROM Items GROUP BY 0 ORDER BY Bucket PIVOT Id", Default::default()).unwrap().query());
    let mut rows = cross("[Id]");
    rows[4].extra = Some(0);
    assert!(compile_read(&rows).is_err());
    let mut rows = cross("[Id]");
    rows.push(Fragment {
        flag: Some(64),
        ..row(3)
    });
    assert!(compile_read(&rows).is_err());
}
#[test]
fn crosstab_reconstruction_checks_limits_and_late_cancellation_before_publication() {
    let rows = cross("[Id] IN (3,1,4)");
    for limits in [
        ParseLimits {
            max_bytes: 32,
            ..Default::default()
        },
        ParseLimits {
            max_tokens: 2,
            ..Default::default()
        },
    ] {
        assert!(
            reconstruct_saved_read(definition(&rows, 0x10), limits, &mut budget(), &|| false)
                .is_err()
        );
    }
    for mut budget in [
        ResourceBudget::new(0, 100, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 0, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 100, 0),
    ] {
        assert!(matches!(
            reconstruct_saved_read(
                definition(&rows, 0x10),
                Default::default(),
                &mut budget,
                &|| false
            ),
            Err(E::Limit(_))
        ));
    }
    let calls = Cell::new(0usize);
    reconstruct_saved_read(
        definition(&rows, 0x10),
        Default::default(),
        &mut budget(),
        &|| {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    for stop in [1, calls.get() / 2, calls.get()] {
        let seen = Cell::new(0usize);
        assert!(matches!(
            reconstruct_saved_read(
                definition(&rows, 0x10),
                Default::default(),
                &mut budget(),
                &|| {
                    seen.set(seen.get() + 1);
                    seen.get() == stop
                }
            ),
            Err(E::Cancelled)
        ));
    }
}

#[test]
fn native_top_percent_and_ties_apply_after_complete_crosstab_cells() {
    for code in [1, 2, 5, 6] {
        for (flags, count, tied, expected) in
            [(16, "1", true, vec![1, 3]), (48, "34", false, vec![1, 2])]
        {
            let mut rows = cross("[Id]");
            rows[2].expression = Some("[Id]");
            rows[6].expression = Some("[Id]");
            if tied {
                rows[8].expression = Some("[Id] Mod 2");
                rows[8].name1 = Some("D");
            }
            rows.push(Fragment {
                flag: Some(flags),
                name1: Some(count),
                ..row(3)
            });
            let mut bytes = database_with_query_flags(&rows, 0x10);
            bytes[20] = code;
            if code == 1 {
                bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
            }
            let before = bytes.clone();
            let mut source = NativeSource::new(
                Cursor::new(bytes.as_slice()),
                CatalogScope::new(7),
                budget(),
                &|| false,
            )
            .unwrap();
            let saved = source
                .read_saved_query("Saved", Default::default())
                .unwrap();
            let result = prepare(saved.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .unwrap();
            assert_eq!(result.columns(), ["Bucket", "Total", "1", "2", "3"]);
            assert_eq!(
                result
                    .rows()
                    .iter()
                    .map(|row| match row[0] {
                        Value::Int32(id) => id,
                        _ => panic!(),
                    })
                    .collect::<Vec<_>>(),
                expected
            );
            drop(source);
            assert_eq!(bytes, before);
        }
    }
}
