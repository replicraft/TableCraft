//! Typed input pays for names and ordinal text before comparing or parsing them.
use super::{Error, Value, updates};
use tablecraft_ace::InspectError;
use tablecraft_core::limits::{Resource, ResourceBudget};

type Cell = (usize, Value, Value);

fn parse<'a>(
    cells: &'a [String],
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<updates::Group<'a, Cell>>, Error> {
    updates::parse(
        cells,
        32,
        |ordinal, expected, value| (ordinal, expected, value),
        work,
        cancelled,
    )
}

#[test]
fn name_comparison_pays_bytes_before_work_and_preserves_cancellation() {
    let left = "A".repeat(16_000);
    let right = "a".repeat(16_000);
    let mut exact = ResourceBudget::new(0, 0, 16_001);
    assert!(updates::same_name(&left, &right, &mut exact, &|| false).unwrap());
    assert_eq!(exact.remaining(Resource::Steps), 0);

    let mut short = ResourceBudget::new(0, 0, 16_000);
    assert!(matches!(
        updates::same_name(&left, &right, &mut short, &|| false),
        Err(Error::Native(InspectError::Limit(error))) if error.resource == Resource::Steps
    ));

    let mut cancelled = ResourceBudget::new(0, 0, 0);
    assert!(matches!(
        updates::same_name(&left, &right, &mut cancelled, &|| true),
        Err(Error::Native(InspectError::Cancelled))
    ));
}

#[test]
fn long_group_names_keep_case_matching_with_exact_paid_work() {
    let table = format!("Items{}", "A".repeat(4_096));
    let column = format!("Score{}", "B".repeat(4_096));
    let cells = vec![
        table.clone(),
        column.clone(),
        "0".to_owned(),
        "int32".to_owned(),
        "10".to_owned(),
        "11".to_owned(),
        table.to_ascii_lowercase(),
        column.to_ascii_lowercase(),
        "1".to_owned(),
        "int32".to_owned(),
        "20".to_owned(),
        "21".to_owned(),
    ];
    let mut baseline = ResourceBudget::new(1_048_576, 10, 100_000);
    let groups = parse(&cells, &mut baseline, &|| false).unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].table, table);
    assert_eq!(groups[0].column, column);
    assert_eq!(
        groups[0].rows,
        vec![
            (0, Value::Int32(10), Value::Int32(11)),
            (1, Value::Int32(20), Value::Int32(21)),
        ]
    );

    let spent = 100_000 - baseline.remaining(Resource::Steps);
    assert!(spent > (table.len() + column.len()) as u64);
    let mut exact = ResourceBudget::new(1_048_576, 10, spent);
    assert_eq!(parse(&cells, &mut exact, &|| false).unwrap().len(), 1);
    assert_eq!(exact.remaining(Resource::Steps), 0);

    let mut short = ResourceBudget::new(1_048_576, 10, spent - 1);
    assert!(matches!(
        parse(&cells, &mut short, &|| false),
        Err(Error::Native(InspectError::Limit(error))) if error.resource == Resource::Steps
    ));
}

#[test]
fn long_ordinal_text_is_paid_before_integer_parsing() {
    let cells = vec![
        "Items".to_owned(),
        "Score".to_owned(),
        "0".repeat(8_192),
        "int32".to_owned(),
        "10".to_owned(),
        "11".to_owned(),
    ];
    let mut work = ResourceBudget::new(1_048_576, 10, 100_000);
    let groups = parse(&cells, &mut work, &|| false).unwrap();
    assert_eq!(groups[0].rows[0].0, 0);
    assert!(100_000 - work.remaining(Resource::Steps) > 8_192);

    let mut short = ResourceBudget::new(1_048_576, 10, 8_191);
    assert!(matches!(
        parse(&cells, &mut short, &|| false),
        Err(Error::Native(InspectError::Limit(error))) if error.resource == Resource::Steps
    ));
}
