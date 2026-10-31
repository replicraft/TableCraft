//! Additional source/expected pairs reuse this target's actual native MR2 owner.
use super::{guard_sources::model, inspect};
use std::io::Cursor;
use tablecraft_ace::{
    MacroProgramCoverage, MacroProgramProfile, NativeImage, TableExtraCoverage, check_table,
    check_table_constraints, read_table,
};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;
const DATA_PAGE: usize = 12;
const SECOND_SLOT_DIRECTORY: usize = 16;
const SCORE_FIELD_OFFSET: usize = 6;
const CASES: [(&str, &str, Option<i32>); 4] = [
    (
        "UPDATE Items SET Score=99 WHERE Id=2",
        "<Statements><Action Name=\"SetField\"><Argument Name=\"Field\">Score</Argument><Argument Name=\"Value\">[Score]+[Id]</Argument></Action></Statements>",
        Some(101),
    ),
    (
        "UPDATE Items SET Score=NULL WHERE Id=2",
        "<Statements><Action Name=\"SetField\"><Argument Name=\"Field\">Score</Argument><Argument Name=\"Value\">[Id]</Argument></Action></Statements>",
        Some(2),
    ),
    (
        "UPDATE Items SET Score=20 WHERE Id=2",
        "<Statements><Action Name=\"SetField\"><Argument Name=\"Field\">Score</Argument><Argument Name=\"Value\">[Old].[Score]</Argument></Action></Statements>",
        Some(20),
    ),
    (
        "UPDATE Items SET Score=99 WHERE Id<=2",
        "<Statements><Action Name=\"SetField\"><Argument Name=\"Field\">Score</Argument><Argument Name=\"Value\">[Score]+[Id]</Argument></Action><ConditionalBlock><If><Condition>[Id]=2</Condition><Statements><Action Name=\"RaiseError\"><Argument Name=\"Number\">17</Argument><Argument Name=\"Description\">rejected</Argument></Action></Statements></If></ConditionalBlock></Statements>",
        None,
    ),
];

fn expected(before: &[u8], final_score: Option<i32>) -> Vec<u8> {
    let mut after = before.to_vec();
    if let Some(score) = final_score {
        let page = &before[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES];
        let start = usize::from(u16::from_le_bytes(
            page[SECOND_SLOT_DIRECTORY..SECOND_SLOT_DIRECTORY + 2]
                .try_into()
                .unwrap(),
        ));
        assert_eq!(start & 0xe000, 0);
        assert_eq!(&page[start..start + 11], model::original_record(1));
        let offset = DATA_PAGE * PAGE_BYTES + start + SCORE_FIELD_OFFSET;
        after[offset..offset + 4].copy_from_slice(&score.to_le_bytes());
    }
    after
}

fn qualify_view(bytes: &[u8], score: i32, bag: &[u8]) {
    let scope = CatalogScope::new(7);
    let image = NativeImage::capture(
        &mut Cursor::new(bytes),
        scope,
        &mut inspect::budget(),
        &|| false,
    )
    .unwrap();
    let checked = check_table(
        &mut image.reader(),
        scope,
        "Items",
        &mut inspect::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 3);
    check_table_constraints(
        &mut image.reader(),
        scope,
        "Items",
        &mut inspect::budget(),
        &|| false,
    )
    .unwrap();
    let rows = read_table(
        &mut image.reader(),
        scope,
        "Items",
        &mut inspect::budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        rows.rows(),
        [
            vec![Value::Int32(1), Value::Int32(10)],
            vec![Value::Int32(2), Value::Int32(score)],
            vec![Value::Int32(3), Value::Int32(30)],
        ]
    );
    let extra = inspect::extra(&image).unwrap();
    assert_eq!(extra.coverage(), TableExtraCoverage::WideMr2);
    assert_eq!(extra.raw(), Some(bag));
    let program = extra.groups()[0].properties().unwrap()[0]
        .inspect_macro_program(
            MacroProgramProfile::BeforeChangeFields,
            &mut inspect::budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(
        program.coverage(),
        MacroProgramCoverage::RecognizedDesktopSubset
    );
    assert_eq!(image.as_bytes(), bytes);
}

#[test]
fn qualify_before_change_sql_sources_and_expected_before_shared_preview() {
    let mut pairs = 0;
    for code in [5, 6] {
        for (_, body, score) in CASES {
            let (before, _, bag) = model::source(code, "BeforeChange", body);
            let after = expected(&before, score);
            qualify_view(&before, 20, &bag);
            qualify_view(&after, score.unwrap_or(20), &bag);
            assert_eq!(
                &before[4 * PAGE_BYTES..5 * PAGE_BYTES],
                &after[4 * PAGE_BYTES..5 * PAGE_BYTES]
            );
            pairs += 1;
        }
    }
    assert_eq!(pairs, 8);
}

#[path = "before_change_sources/physical_roles.rs"]
mod physical_roles;

#[path = "before_change_sources/api.rs"]
mod api;

#[path = "before_change_sources/cli.rs"]
mod cli;
