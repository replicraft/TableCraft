use super::joins::{join, source};
use super::*;
use tablecraft_access::reconstruct_saved_read;
use tablecraft_core::values::Value;
use tablecraft_query::{
    prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};

fn aliases(count: usize) -> Vec<String> {
    (0..count).map(|index| format!("a{index}")).collect()
}

fn tables<'a>(names: &'a [String]) -> Vec<Fragment<'a>> {
    names
        .iter()
        .map(|name| source("Items", Some(name)))
        .collect()
}

fn one_row() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "Items".into(),
            vec![Column::new("ID".into(), ScalarType::Int32, false).unwrap()],
            vec![vec![Value::Int32(7)]],
        )
        .unwrap(),
    ])
    .unwrap()
}

#[test]
fn seventeen_and_thirty_two_comma_sources_reconstruct_and_execute_the_last_reference() {
    for count in [17, 32] {
        let names = aliases(count);
        let expression = format!("[a{}].[ID]", count - 1);
        let mut rows = tables(&names);
        rows.push(column(&expression));
        let saved = compile(&rows).unwrap();
        let expected = format!(
            "SELECT {expression} FROM {}",
            names
                .iter()
                .map(|name| format!("Items AS {name}"))
                .collect::<Vec<_>>()
                .join(",")
        );
        assert_eq!(
            saved.query(),
            parse_query(&expected, ParseLimits::default())
                .unwrap()
                .query()
        );
        let source = one_row();
        let result = prepare(saved.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap();
        assert_eq!(result.rows(), &[vec![Value::Int32(7)]]);
    }
}

#[test]
fn sixteen_disconnected_joins_cover_all_thirty_two_member_bits() {
    let names = aliases(32);
    let conditions = (0..16)
        .map(|index| format!("[a{index}].[ID]=[a{}].[ID]", 31 - index))
        .collect::<Vec<_>>();
    let orders = (0_u8..16).map(|index| [index]).collect::<Vec<_>>();
    let mut rows = tables(&names);
    rows.push(column("[a31].[ID]"));
    let mut roots = Vec::new();
    for index in 0..16 {
        rows.push(Fragment {
            order: Some(&orders[index]),
            ..join(1, &names[index], &names[31 - index], &conditions[index])
        });
        roots.push(format!(
            "Items AS a{index} INNER JOIN Items AS a{} ON {}",
            31 - index,
            conditions[index]
        ));
    }
    let saved = compile(&rows).unwrap();
    let expected = format!("SELECT a31.ID FROM {}", roots.join(","));
    assert_eq!(
        saved.query(),
        parse_query(&expected, ParseLimits::default())
            .unwrap()
            .query()
    );
    let source = one_row();
    assert_eq!(
        prepare(saved.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        &[vec![Value::Int32(7)]]
    );

    // A seventeenth fragment remains outside the qualified join ceiling.
    rows.push(join(1, "a0", "a31", "True"));
    assert_eq!(compile(&rows).err(), Some(E::SyntaxLimit));
}

#[test]
fn crosstab_shares_the_thirty_two_source_boundary_and_refuses_thirty_three() {
    for count in [32, 33] {
        let names = aliases(count);
        let mut rows = tables(&names);
        rows.extend([
            Fragment {
                flag: Some(6),
                ..row(1)
            },
            Fragment {
                flag: Some(2),
                name1: Some("Bucket"),
                ..column("0")
            },
            Fragment {
                flag: Some(0),
                ..column("Count(*)")
            },
            Fragment {
                flag: Some(1),
                ..column("a31.ID IN(7)")
            },
            Fragment {
                flag: Some(2),
                expression: Some("0"),
                ..row(9)
            },
            Fragment {
                flag: Some(1),
                expression: Some("a31.ID"),
                ..row(9)
            },
        ]);
        let result = reconstruct_saved_read(
            definition(&rows, 0x10),
            ParseLimits::default(),
            &mut budget(),
            &|| false,
        );
        if count == 33 {
            assert_eq!(result.err(), Some(E::Unsupported));
            continue;
        }
        let saved = result.unwrap();
        let source = one_row();
        assert_eq!(
            prepare(saved.query(), &source)
                .unwrap()
                .execute(&source, &[], Default::default())
                .unwrap()
                .rows(),
            &[vec![Value::Int64(0), Value::Int64(1)]]
        );
    }
}

#[test]
fn expanded_source_boundary_keeps_syntax_memory_work_and_cancellation_limits() {
    let names = aliases(32);
    let mut rows = tables(&names);
    rows.push(column("a31.ID"));
    for mut allowance in [
        ResourceBudget::new(1, 1000, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 1000, 1),
    ] {
        assert!(matches!(
            reconstruct_saved_select(
                definition(&rows, 0),
                ParseLimits::default(),
                &mut allowance,
                &|| false,
            ),
            Err(E::Limit(_))
        ));
    }
    assert!(matches!(
        reconstruct_saved_select(
            definition(&rows, 0),
            ParseLimits {
                max_bytes: 64,
                ..ParseLimits::default()
            },
            &mut budget(),
            &|| false,
        ),
        Err(E::SyntaxLimit) | Err(E::Parse(_))
    ));

    let checks = Cell::new(0);
    reconstruct_saved_select(
        definition(&rows, 0),
        ParseLimits::default(),
        &mut budget(),
        &|| {
            checks.set(checks.get() + 1);
            false
        },
    )
    .unwrap();
    let total = checks.get();
    for stop in [1, total / 2, total] {
        let checks = Cell::new(0);
        assert_eq!(
            reconstruct_saved_select(
                definition(&rows, 0),
                ParseLimits::default(),
                &mut budget(),
                &|| {
                    checks.set(checks.get() + 1);
                    checks.get() == stop
                },
            )
            .err(),
            Some(E::Cancelled)
        );
    }
}
