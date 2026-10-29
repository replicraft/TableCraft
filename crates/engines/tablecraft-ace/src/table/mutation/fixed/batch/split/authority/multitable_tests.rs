//! Original groups and the cumulative GLOBAL overlay remain independent authority.
use crate::{
    EditError, FixedColumnUpdates, FixedScalarUpdate, NativeImage, TableCheck,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        reader::Reader,
    },
    table::{
        indexes::{
            IndexKeyUpdate, SplitIndexPlan,
            mutation::split::fixed_batch::{self, Input},
        },
        mutation::{
            append::owners::MultipleOwners,
            fixed::{batch::multitable_tests::model, masked},
            gates, index_append,
            patch::spans,
        },
    },
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1024 * 1024 * 1024, 100_000, 1_000_000_000)
}

fn fixture() -> model::Case {
    model::source(model::Settings {
        code: 6,
        descending: true,
        unique: false,
        forwarded: true,
        mode: model::Mode::Full,
    })
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn assignments(case: &model::Case) -> Vec<Vec<FixedScalarUpdate>> {
    let mut result = Vec::new();
    for table in &case.tables {
        for right in [false, true] {
            result.push(
                table
                    .original
                    .iter()
                    .zip(&table.final_rows)
                    .enumerate()
                    .map(|(ordinal, (old, new))| {
                        let (expected, value) = if right {
                            (old.right, new.right)
                        } else {
                            (old.left, new.left)
                        };
                        FixedScalarUpdate::new(
                            ordinal,
                            expected.map_or(Value::Null, Value::Int32),
                            value.map_or(Value::Null, Value::Int32),
                        )
                    })
                    .collect(),
            );
        }
    }
    result
}

fn groups<'a>(
    case: &model::Case,
    updates: &'a [Vec<FixedScalarUpdate>],
) -> Vec<FixedColumnUpdates<'a>> {
    case.tables
        .iter()
        .enumerate()
        .flat_map(|(position, table)| {
            [
                FixedColumnUpdates::new(table.name, "Group", &updates[position * 2]),
                FixedColumnUpdates::new(table.name, "Score", &updates[position * 2 + 1]),
            ]
        })
        .collect()
}

struct Staged {
    checked: Vec<TableCheck>,
    fields: Vec<spans::Patch>,
    plan: SplitIndexPlan<MultipleOwners>,
    after: Vec<u8>,
}

fn stage(image: &NativeImage, groups: &[FixedColumnUpdates<'_>]) -> Staged {
    let mut checked = Vec::new();
    let mut keys: Vec<Vec<IndexKeyUpdate>> = Vec::new();
    let mut fields = Vec::new();
    for group in groups {
        let position = if let Some(position) = checked.iter().position(|table: &TableCheck| {
            table.schema().name().eq_ignore_ascii_case(group.table())
        }) {
            position
        } else {
            checked.push(
                gates::before_fixed_update(image, group.table(), &mut budget(), &|| false).unwrap(),
            );
            keys.push(Vec::new());
            checked.len() - 1
        };
        let staged = masked::stage::<4, true>(
            image,
            &checked[position],
            group.column(),
            group.updates(),
            ScalarType::Int32,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        for patch in &staged.patches {
            spans::push(&mut fields, patch, &mut budget(), &|| false).unwrap();
        }
        keys[position].extend(staged.keys);
    }
    spans::coalesce(&mut fields, &mut budget(), &|| false).unwrap();
    let inputs = checked
        .iter()
        .zip(&keys)
        .map(|(checked, updates)| Input { checked, updates })
        .collect::<Vec<_>>();
    let plan = fixed_batch::multiple(image, &inputs, &mut budget(), &|| false).unwrap();
    let after = index_append::compose(image, &fields, &plan, &mut budget(), &|| false).unwrap();
    Staged {
        checked,
        fields,
        plan,
        after,
    }
}

fn prove(
    image: &NativeImage,
    groups: &[FixedColumnUpdates<'_>],
    staged: &Staged,
) -> Result<usize, EditError> {
    let tables = staged.checked.iter().collect::<Vec<_>>();
    super::prove(
        image,
        &tables,
        groups,
        &staged.plan,
        &staged.after,
        &mut budget(),
        &|| false,
    )
}

fn healthy(case: &model::Case, image: &NativeImage, groups: &[FixedColumnUpdates<'_>]) -> Staged {
    let staged = stage(image, groups);
    assert_eq!(staged.after.len(), case.expected.len());
    assert!(
        staged.after == case.expected,
        "complete independent literal image differs"
    );
    assert_eq!(prove(image, groups, &staged).unwrap(), 1000);
    staged
}

#[test]
fn earlier_table_global_bits_cannot_be_dropped_from_a_correlated_plan_and_candidate() {
    let case = fixture();
    let updates = assignments(&case);
    let groups = groups(&case, &updates);
    let image = capture(&case.before);
    let mut staged = healthy(&case, &image, &groups);
    let mut source = image.reader();
    let mut pool = budget();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget: &mut pool,
        cancelled: &|| false,
        reads: 0,
    };
    let global =
        RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree).unwrap();
    let mut modified = 0;
    for page in case.added.iter().filter(|page| page.definition == 10) {
        let cell = global.future_cell(&mut reader, page.page).unwrap();
        let edit = staged
            .plan
            .append
            .map_edits
            .iter_mut()
            .find(|edit| edit.offset == cell.offset)
            .unwrap();
        assert_eq!(edit.after & cell.mask, 0);
        edit.after |= cell.mask;
        staged.after[cell.offset] |= cell.mask;
        modified += 1;
    }
    assert_eq!(modified, 4);
    let recomposed =
        index_append::compose(&image, &staged.fields, &staged.plan, &mut budget(), &|| {
            false
        })
        .unwrap();
    assert!(
        recomposed == staged.after,
        "forged candidate and map ledger must agree"
    );
    assert_eq!(
        prove(&image, &groups, &staged),
        Err(EditError::Preservation)
    );
}

#[test]
fn internally_valid_two_table_population_does_not_replace_original_groups() {
    let case = fixture();
    let updates = assignments(&case);
    let original_groups = groups(&case, &updates);
    let image = capture(&case.before);
    healthy(&case, &image, &original_groups);

    let mut altered = assignments(&case);
    let value = case.tables[0].final_rows[0].left.unwrap();
    altered[0][0] =
        FixedScalarUpdate::new(0, Value::Null, Value::Int32(value.checked_add(1).unwrap()));
    let alternative_groups = groups(&case, &altered);
    let alternative = stage(&image, &alternative_groups);
    assert_eq!(
        prove(&image, &alternative_groups, &alternative).unwrap(),
        1000
    );
    assert_eq!(
        prove(&image, &original_groups, &alternative),
        Err(EditError::Preservation)
    );
}
