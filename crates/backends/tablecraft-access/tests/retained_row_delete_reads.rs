//! Independent target-aware DELETE retains physical history and generated counters.
use std::{fs, io::Cursor};
use tablecraft_access::NativeSource;
use tablecraft_ace::{check_table, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, prepare};
use tablecraft_sql::parse_query;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::{
        current_delete,
        deleted_field::populated::{CASES, Case, readdition::Target as Field},
    },
    canonical_retained_add_table::{
        self as raw, Target, deletes,
        rows::{self, Input, Profile},
        updates,
    },
    canonical_table_drop::current as dropped,
    raw_forwarding_chain,
};

const PAGE: usize = 4096;
const PROFILES: [(Profile, Target, bool); 4] = [
    (Profile::NativeNumber, Target::GeneratedNumber, false),
    (Profile::NativeText, Target::GeneratedText, false),
    (Profile::DesktopNumber, Target::GeneratedNumber, true),
    (Profile::DesktopText, Target::GeneratedText, true),
];

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

fn numeric(profile: Profile) -> bool {
    matches!(profile, Profile::NativeNumber | Profile::DesktopNumber)
}

fn observe(bytes: &[u8], table: &str, expected: &[Vec<Value>]) {
    let scope = CatalogScope::new(3421);
    assert_eq!(
        read_table(
            &mut Cursor::new(bytes),
            scope,
            table,
            &mut budget(),
            &|| false
        )
        .unwrap()
        .rows(),
        expected
    );
    check_table(
        &mut Cursor::new(bytes),
        scope,
        table,
        &mut budget(),
        &|| false,
    )
    .unwrap();
}

fn source(
    case: Case,
    profile: Profile,
    target: Target,
    desktop: bool,
    second: bool,
) -> (Vec<u8>, u32, Vec<Vec<Value>>) {
    let old = dropped::source(case, Field::ShortText, true, dropped::History::Inserted);
    let mut before = dropped::after(&old);
    if second {
        before = raw::append(&before, "Other", Target::GeneratedNumber);
        before = rows::append(&before, 11, Profile::NativeNumber, Input::Number(99));
    }
    let definition = (before.len() / PAGE) as u32;
    let mut bytes = if desktop {
        raw::desktop::append(&before, "Table1", target)
    } else {
        raw::append(&before, "Table1", target)
    };
    let seeds = if numeric(profile) {
        [Input::Number(17), Input::Null, Input::Number(-7)]
    } else {
        [
            Input::Text("first"),
            Input::Text(if desktop { "" } else { "second" }),
            Input::Text("opaque"),
        ]
    };
    let mut expected = Vec::new();
    for (position, input) in seeds.into_iter().enumerate() {
        bytes = rows::append(&bytes, definition, profile, input);
        expected.push(rows::row(profile, input, (position + 1) as i32));
    }
    if numeric(profile) {
        bytes = updates::number(&bytes, definition, 0, None);
        expected[0][1] = Value::Null;
    } else {
        let text = "longer forwarded text 日本語🦀";
        bytes = updates::text(&bytes, definition, 0, Some(text));
        bytes = updates::with_opaque_text_null(&bytes, definition, 2);
        expected[0][1] = Value::Text(text.into());
        expected[2][1] = Value::Null;
    }
    (bytes, definition, expected)
}

fn retained_bytes(before: &[u8], after: &[u8], definition: u32, survivors: usize) {
    let td = definition as usize * PAGE;
    let data = td + PAGE;
    let index = data + PAGE;
    assert_eq!(&after[..td], &before[..td]);
    assert_eq!(&after[td..td + 16], &before[td..td + 16]);
    assert_eq!(&after[td + 20..data], &before[td + 20..data]);
    assert_eq!(&after[index + PAGE..], &before[index + PAGE..]);
    let physical = word(before, data + 12);
    let frontier = 14 + 2 * physical + word(before, data + 2);
    assert_eq!(
        &after[data + frontier..index],
        &before[data + frontier..index]
    );
    assert_eq!(&after[data..data + 14], &before[data..data + 14]);
    assert_eq!(
        &after[index + 480 + survivors * 9..index + PAGE],
        &before[index + 480 + survivors * 9..index + PAGE]
    );
}

#[test]
fn retained_deletion_subsets_permutations_all_and_empty_keep_opaque_storage() {
    for case in CASES {
        for (profile, target, desktop) in PROFILES {
            for second in [false, true] {
                let (before, definition, expected) = source(case, profile, target, desktop, second);
                for removed in [vec![], vec![0], vec![1], vec![2], vec![0, 2], vec![0, 1, 2]] {
                    let after = deletes::after(&before, definition, &removed);
                    let mut reversed = removed.to_vec();
                    reversed.reverse();
                    assert_eq!(deletes::after(&before, definition, &reversed), after);
                    let survivors = expected
                        .iter()
                        .enumerate()
                        .filter(|(root, _)| !removed.contains(root))
                        .map(|(_, row)| row.to_vec())
                        .collect::<Vec<_>>();
                    retained_bytes(&before, &after, definition, survivors.len());
                    observe(&after, "Table1", &survivors);
                    if second {
                        observe(&after, "Other", &[vec![Value::Int32(1), Value::Int32(99)]]);
                    }
                    if removed.is_empty() {
                        assert_eq!(after, before);
                    }
                }
            }
        }
    }
}

#[test]
fn empty_then_insert_keeps_issued_ids_and_appends_after_the_actual_physical_frontier() {
    for case in CASES {
        for (profile, target, desktop) in PROFILES {
            let (before, definition, _) = source(case, profile, target, desktop, true);
            let partial = deletes::after(&before, definition, &[0, 2]);
            let empty = deletes::after(&partial, definition, &[1]);
            observe(&empty, "Table1", &[]);
            let physical = word(&empty, (definition as usize + 1) * PAGE + 12);
            let supplied = if numeric(profile) {
                Input::Number(123)
            } else {
                Input::Text("new 日本語🦀")
            };
            let inserted = rows::append(&empty, definition, profile, supplied);
            let expected = vec![rows::row(profile, supplied, 4)];
            observe(&inserted, "Table1", &expected);
            let empty_again = deletes::after(&inserted, definition, &[physical]);
            observe(&empty_again, "Table1", &[]);
            let next = rows::append(&empty_again, definition, profile, supplied);
            observe(&next, "Table1", &[rows::row(profile, supplied, 5)]);
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("reinserted.accdb");
            fs::write(&path, &next).unwrap();
            for scope in [3422, 3423] {
                let source = NativeSource::new(
                    fs::File::open(&path).unwrap(),
                    CatalogScope::new(scope),
                    budget(),
                    &|| false,
                )
                .unwrap();
                let query =
                    parse_query("SELECT * FROM Table1 ORDER BY ID", Default::default()).unwrap();
                assert_eq!(
                    prepare(query.query(), &source)
                        .unwrap()
                        .execute(&source, &[], ExecutionOptions::default())
                        .unwrap()
                        .rows(),
                    &[rows::row(profile, supplied, 5)]
                );
            }
            assert_eq!(fs::read(path).unwrap(), next);
        }
    }
}

#[test]
fn long_forwarded_user_and_catalog_roots_can_be_deleted_without_rewriting_terminals() {
    let (before, definition, expected) = source(
        Case::MixedBinary,
        Profile::DesktopText,
        Target::GeneratedText,
        true,
        true,
    );
    let catalog_root = word(&before, 4 * PAGE + 12) - 1;
    let user = raw_forwarding_chain::extend(&before, definition + 1, 0, 20);
    let before = raw_forwarding_chain::extend(&user, 4, catalog_root, 20);
    let witness = deletes::record(&before, definition, 0).to_vec();
    assert!(!witness.is_empty());
    let after = deletes::after(&before, definition, &[0]);
    retained_bytes(&before, &after, definition, 2);
    observe(&after, "Table1", &expected[1..]);
    let empty = deletes::after(&after, definition, &[1, 2]);
    observe(&empty, "Table1", &[]);
}

#[test]
fn original_sparse_delete_commutes_with_sibling_append_and_new_targets_above_page_sixty_three() {
    for case in CASES {
        let source = current_delete::source(case, Field::ShortText, true, true, true);
        let sibling = raw::desktop::append(&source.bytes, "More", Target::GeneratedNumber);
        let old = current_delete::after(&source.bytes, &[source.roots[0]]);
        let expected = raw::desktop::append(&old, "More", Target::GeneratedNumber);
        assert_eq!(&expected[11 * PAGE..], &sibling[11 * PAGE..]);
        observe(
            &expected,
            "Table1",
            &source
                .survivors(&[source.roots[0]])
                .map(<[Value]>::to_vec)
                .collect::<Vec<_>>(),
        );
    }
    let old = dropped::source(
        Case::TextPresent,
        Field::ShortText,
        true,
        dropped::History::Fields,
    );
    let mut bytes = dropped::after(&old);
    let mut definition = 0;
    for ordinal in 0..19 {
        definition = (bytes.len() / PAGE) as u32;
        bytes = raw::append(&bytes, &format!("T{ordinal}"), Target::GeneratedNumber);
    }
    assert!(definition > 63);
    bytes = rows::append(&bytes, definition, Profile::NativeNumber, Input::Number(17));
    let after = deletes::after(&bytes, definition, &[0]);
    retained_bytes(&bytes, &after, definition, 0);
    observe(&after, "T18", &[]);
}
