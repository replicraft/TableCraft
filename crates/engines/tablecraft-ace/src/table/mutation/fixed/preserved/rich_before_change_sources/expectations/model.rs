//! Literal original/request/XML outcomes; no production preparation or encoder.
use super::{fixed_model, required_sources, sources};
use fixed_model::{
    Assignment as FixedAssignment, Cell,
    Cell::{Boolean as B, Currency as C, Date as D, Int32 as I, Int64 as L, Null as N},
    Column,
    Column::{Currency, Date, Large, LargeInt32, Time, Yes},
    Group,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Recipe {
    RichWideRows,
    RichDateBits,
    SequentialSeeded,
    SequentialWideOnly,
    SequentialUnionRows,
    SequentialRestoresOriginal,
    SequentialNullSeeds,
    DerivedOnlyWide,
    DerivedOnlyOverwritesUser,
    DerivedOnlyOnNoop,
    DormantDeleteWide,
    CounterOperand,
    AllNoops,
    EntirelyEmpty,
    LateRaise,
    DormantIndexedTarget,
    DormantDependentTarget,
    OpaqueFieldRead,
    WideFieldRead,
    BodyPresent,
    ModulePresent,
    OpaqueExtra,
    UnselectedProperty,
    ChildPresent,
    CounterTarget,
    ComplexCurrentRead,
    ComplexOldRead,
    RequiredRepair,
    RequiredUnrepaired,
}

pub(crate) const RECIPES: [Recipe; 29] = [
    Recipe::RichWideRows,
    Recipe::RichDateBits,
    Recipe::SequentialSeeded,
    Recipe::SequentialWideOnly,
    Recipe::SequentialUnionRows,
    Recipe::SequentialRestoresOriginal,
    Recipe::SequentialNullSeeds,
    Recipe::DerivedOnlyWide,
    Recipe::DerivedOnlyOverwritesUser,
    Recipe::DerivedOnlyOnNoop,
    Recipe::DormantDeleteWide,
    Recipe::CounterOperand,
    Recipe::AllNoops,
    Recipe::EntirelyEmpty,
    Recipe::LateRaise,
    Recipe::DormantIndexedTarget,
    Recipe::DormantDependentTarget,
    Recipe::OpaqueFieldRead,
    Recipe::WideFieldRead,
    Recipe::BodyPresent,
    Recipe::ModulePresent,
    Recipe::OpaqueExtra,
    Recipe::UnselectedProperty,
    Recipe::ChildPresent,
    Recipe::CounterTarget,
    Recipe::ComplexCurrentRead,
    Recipe::ComplexOldRead,
    Recipe::RequiredRepair,
    Recipe::RequiredUnrepaired,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Refusal {
    HeaderProfile,
    SourcePayload,
    IncompleteSelection,
    FieldBinding,
    Required,
    Rejected { row: usize, number: i32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Assignment {
    pub row: usize,
    pub column: usize,
    pub value: Cell,
}

pub(crate) struct Case {
    pub source: sources::Case,
    pub groups: Vec<Group>,
    pub expected: Option<Vec<u8>>,
    pub changed: usize,
    pub event_rows: Vec<usize>,
    pub assignments: Vec<Assignment>,
    pub refusal: Option<Refusal>,
}

fn group(column: Column, updates: &[(usize, Cell, Cell)]) -> Group {
    Group {
        table: "Items",
        name: column.name(),
        column,
        updates: updates
            .iter()
            .map(|&(row, expected, replacement)| FixedAssignment {
                row,
                expected,
                replacement,
            })
            .collect(),
    }
}

fn assignment(row: usize, value: Cell) -> Assignment {
    Assignment {
        row,
        column: 4,
        value,
    }
}

fn source_recipe(recipe: Recipe) -> sources::Recipe {
    match recipe {
        Recipe::RichWideRows | Recipe::RichDateBits | Recipe::AllNoops => {
            sources::Recipe::RichReadGuard
        }
        Recipe::SequentialSeeded
        | Recipe::SequentialWideOnly
        | Recipe::SequentialUnionRows
        | Recipe::SequentialRestoresOriginal
        | Recipe::SequentialNullSeeds
        | Recipe::EntirelyEmpty => sources::Recipe::Int32Sequential,
        Recipe::DerivedOnlyWide | Recipe::DerivedOnlyOverwritesUser | Recipe::DerivedOnlyOnNoop => {
            sources::Recipe::Int32DerivedOnly
        }
        Recipe::DormantDeleteWide => sources::Recipe::DormantDelete,
        Recipe::CounterOperand => sources::Recipe::CounterRead,
        Recipe::LateRaise => sources::Recipe::Int32LateRaise,
        Recipe::DormantIndexedTarget => sources::Recipe::DormantIndexedTarget,
        Recipe::DormantDependentTarget => sources::Recipe::DormantDependentTarget,
        Recipe::OpaqueFieldRead => sources::Recipe::OpaqueFieldRead,
        Recipe::WideFieldRead => sources::Recipe::WideFieldRead,
        Recipe::BodyPresent => sources::Recipe::BodyPresent,
        Recipe::ModulePresent => sources::Recipe::ModulePresent,
        Recipe::OpaqueExtra => sources::Recipe::OpaqueExtra,
        Recipe::UnselectedProperty => sources::Recipe::UnselectedProperty,
        Recipe::ChildPresent => sources::Recipe::ChildPresent,
        Recipe::CounterTarget => sources::Recipe::CounterTarget,
        Recipe::ComplexCurrentRead => sources::Recipe::ComplexCurrentRead,
        Recipe::ComplexOldRead => sources::Recipe::ComplexOldRead,
        Recipe::RequiredRepair => sources::Recipe::Int32DerivedOnly,
        Recipe::RequiredUnrepaired => sources::Recipe::DormantDelete,
    }
}

fn requests(recipe: Recipe) -> Vec<Group> {
    match recipe {
        Recipe::RichWideRows => vec![
            group(Large, &[(0, L(1000), L(-101))]),
            group(Currency, &[(1, C(24690), C(-10001))]),
            group(Time, &[(2, D(0x3fe0000000000000), D(0x3fd0000000000000))]),
            group(Yes, &[(2, B(false), B(true))]),
        ],
        Recipe::RichDateBits => vec![group(Date, &[(0, D(0), D(0x8000000000000000))])],
        Recipe::SequentialSeeded => vec![
            group(LargeInt32, &[(0, I(1000), I(10))]),
            group(Currency, &[(0, C(12345), C(-10001))]),
        ],
        Recipe::SequentialWideOnly | Recipe::DerivedOnlyWide => {
            vec![group(Currency, &[(0, C(12345), C(-10001))])]
        }
        Recipe::SequentialUnionRows => vec![
            group(LargeInt32, &[(0, I(1000), I(10))]),
            group(Currency, &[(1, C(24690), C(-10001))]),
            group(Yes, &[(2, B(false), B(true))]),
        ],
        Recipe::SequentialRestoresOriginal => vec![
            group(LargeInt32, &[(0, I(1000), I(-1))]),
            group(Currency, &[(0, C(12345), C(12345))]),
        ],
        Recipe::SequentialNullSeeds => vec![
            group(LargeInt32, &[(0, I(1000), N)]),
            group(Currency, &[(0, C(12345), N)]),
            group(Yes, &[(0, B(false), B(true))]),
        ],
        Recipe::DerivedOnlyOverwritesUser => vec![
            group(LargeInt32, &[(0, I(1000), I(55))]),
            group(Time, &[(0, D(0x3fe0000000000000), D(0x3fd0000000000000))]),
        ],
        Recipe::DerivedOnlyOnNoop => vec![group(Currency, &[(0, C(12345), C(12345))])],
        Recipe::DormantDeleteWide => vec![
            group(Currency, &[(1, C(24690), C(-10001))]),
            group(Yes, &[(1, B(false), B(true))]),
        ],
        Recipe::CounterOperand => vec![group(Yes, &[(0, B(false), B(true))])],
        Recipe::AllNoops => vec![
            group(Currency, &[(0, C(12345), C(12345))]),
            group(Yes, &[(0, B(false), B(false))]),
        ],
        Recipe::RequiredRepair | Recipe::RequiredUnrepaired => vec![
            group(LargeInt32, &[(0, I(1000), N)]),
            group(Yes, &[(0, B(false), B(true))]),
        ],
        Recipe::EntirelyEmpty => vec![group(LargeInt32, &[]), group(Time, &[]), group(Yes, &[])],
        Recipe::LateRaise => vec![group(
            Currency,
            &[(0, C(12345), C(-10001)), (1, C(24690), C(-20002))],
        )],
        _ => vec![group(Currency, &[(0, C(12345), C(-10001))])],
    }
}

fn literal_assignments(recipe: Recipe) -> Vec<Assignment> {
    match recipe {
        Recipe::SequentialSeeded => vec![assignment(0, I(11)), assignment(0, I(1011))],
        Recipe::SequentialWideOnly => vec![assignment(0, I(1001)), assignment(0, I(2001))],
        Recipe::SequentialUnionRows => vec![
            assignment(0, I(11)),
            assignment(0, I(1011)),
            assignment(1, I(2002)),
            assignment(1, I(4002)),
            assignment(2, I(3003)),
            assignment(2, I(6003)),
        ],
        Recipe::SequentialRestoresOriginal => vec![assignment(0, I(0)), assignment(0, I(1000))],
        Recipe::SequentialNullSeeds => vec![assignment(0, N), assignment(0, N)],
        Recipe::DerivedOnlyWide
        | Recipe::DerivedOnlyOverwritesUser
        | Recipe::DerivedOnlyOnNoop
        | Recipe::CounterOperand
        | Recipe::RequiredRepair => vec![assignment(0, I(1))],
        Recipe::LateRaise => vec![assignment(0, I(1)), assignment(1, I(2))],
        _ => Vec::new(),
    }
}

fn refusal(recipe: Recipe) -> Option<Refusal> {
    match recipe {
        Recipe::RequiredUnrepaired => Some(Refusal::Required),
        Recipe::LateRaise => Some(Refusal::Rejected { row: 1, number: 17 }),
        Recipe::BodyPresent
        | Recipe::ModulePresent
        | Recipe::OpaqueExtra
        | Recipe::ChildPresent => Some(Refusal::SourcePayload),
        Recipe::UnselectedProperty => Some(Refusal::IncompleteSelection),
        Recipe::DormantIndexedTarget
        | Recipe::DormantDependentTarget
        | Recipe::OpaqueFieldRead
        | Recipe::WideFieldRead
        | Recipe::CounterTarget
        | Recipe::ComplexCurrentRead
        | Recipe::ComplexOldRead => Some(Refusal::FieldBinding),
        _ => None,
    }
}

fn literal_final_groups(before: &[u8], groups: &[Group], assignments: &[Assignment]) -> Vec<Group> {
    let mut final_cells: BTreeMap<usize, (Column, BTreeMap<usize, Cell>)> = BTreeMap::new();
    for group in groups {
        for update in &group.updates {
            assert_eq!(
                fixed_model::read(before, group.column, update.row),
                update.expected
            );
            final_cells
                .entry(group.column.number())
                .or_insert_with(|| (group.column, BTreeMap::new()))
                .1
                .insert(update.row, update.replacement);
        }
    }
    for assignment in assignments {
        assert_eq!(assignment.column, 4);
        final_cells
            .entry(4)
            .or_insert_with(|| (LargeInt32, BTreeMap::new()))
            .1
            .insert(assignment.row, assignment.value);
    }
    final_cells
        .into_values()
        .map(|(column, cells)| {
            group(
                column,
                &cells
                    .into_iter()
                    .map(|(row, replacement)| {
                        (row, fixed_model::read(before, column, row), replacement)
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

fn assert_literal_program(source: &sources::Case, recipe: Recipe) {
    if refusal(recipe).is_some() {
        return;
    }
    let raise = concat!(
        "<ConditionalBlock><If><Condition>[ID]&lt;0</Condition><Statements>",
        "<Action Name=\"RaiseError\"><Argument Name=\"Number\">17</Argument>",
        "<Argument Name=\"Description\">rejected</Argument></Action>",
        "</Statements></If></ConditionalBlock>"
    );
    let set_id = concat!(
        "<Action Name=\"SetField\"><Argument Name=\"Field\">Large_Number</Argument>",
        "<Argument Name=\"Value\">[ID]</Argument></Action>"
    );
    let sequential = concat!(
        "<Action Name=\"SetField\"><Argument Name=\"Field\">Large_Number</Argument>",
        "<Argument Name=\"Value\">[Large_Number]+[ID]</Argument></Action>",
        "<Action Name=\"SetField\"><Argument Name=\"Field\">Large_Number</Argument>",
        "<Argument Name=\"Value\">[Large_Number]+[Old].[Large_Number]</Argument></Action>"
    );
    let body = match recipe {
        Recipe::SequentialSeeded
        | Recipe::SequentialWideOnly
        | Recipe::SequentialUnionRows
        | Recipe::SequentialRestoresOriginal
        | Recipe::SequentialNullSeeds
        | Recipe::EntirelyEmpty => sequential,
        Recipe::DerivedOnlyWide
        | Recipe::DerivedOnlyOverwritesUser
        | Recipe::DerivedOnlyOnNoop
        | Recipe::CounterOperand
        | Recipe::RequiredRepair => set_id,
        _ => raise,
    };
    let delete = if recipe == Recipe::DormantDeleteWide {
        concat!(
            "<DataMacro Event=\"BeforeDelete\"><Statements>",
            "<ConditionalBlock><If><Condition>[Old].[ID]=2</Condition><Statements>",
            "<Action Name=\"RaiseError\"><Argument Name=\"Number\">17</Argument>",
            "<Argument Name=\"Description\">rejected</Argument></Action>",
            "</Statements></If></ConditionalBlock></Statements></DataMacro>"
        )
    } else {
        ""
    };
    let expected = format!(
        "<DataMacros xmlns=\"http://schemas.microsoft.com/office/accessservices/2009/11/application\"><DataMacro Event=\"BeforeChange\"><Statements>{body}</Statements></DataMacro>{delete}</DataMacros>"
    );
    assert_eq!(source.properties, [("StoredGuard".to_owned(), expected)]);
    assert_eq!(source.selection, [0]);
}

pub(crate) fn source(code: u8, recipe: Recipe) -> Case {
    let source = match recipe {
        Recipe::RequiredRepair => required_sources::source(code, true).source,
        Recipe::RequiredUnrepaired => required_sources::source(code, false).source,
        _ => sources::source(code, source_recipe(recipe)),
    };
    assert_literal_program(&source, recipe);
    let groups = requests(recipe);
    let assignments = literal_assignments(recipe);
    let event_rows = groups
        .iter()
        .flat_map(|group| group.updates.iter().map(|update| update.row))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let refusal = refusal(recipe).or((code == 5).then_some(Refusal::HeaderProfile));
    let (expected, changed) = if refusal.is_none() {
        let final_groups = literal_final_groups(&source.before, &groups, &assignments);
        let (bytes, changed) = fixed_model::reconstruct(&source.before, &final_groups);
        let literal_count = match recipe {
            Recipe::RichWideRows | Recipe::SequentialUnionRows => 3,
            Recipe::SequentialRestoresOriginal | Recipe::AllNoops | Recipe::EntirelyEmpty => 0,
            _ => 1,
        };
        assert_eq!(changed, literal_count);
        (Some(bytes), changed)
    } else {
        (None, 0)
    };
    Case {
        source,
        groups,
        expected,
        changed,
        event_rows,
        assignments,
        refusal,
    }
}

pub(crate) fn assert_complement(case: &Case) {
    let Some(expected) = &case.expected else {
        assert!(case.refusal.is_some());
        return;
    };
    let before = &case.source.before;
    assert_eq!(before.len(), expected.len());
    let final_groups = literal_final_groups(before, &case.groups, &case.assignments);
    let mut payload = BTreeSet::new();
    let mut masks = BTreeMap::new();
    let mut changed = BTreeSet::new();
    for group in &final_groups {
        for update in &group.updates {
            let (at, mask) = fixed_model::location(before, group.column, update.row);
            let bit = 1_u8 << (group.column.number() % 8);
            *masks.entry(mask).or_insert(0_u8) |= bit;
            if update.replacement != N {
                payload.extend(at..at + group.column.width());
            }
            if update.expected != update.replacement {
                changed.insert(update.row);
            }
            assert_eq!(
                fixed_model::read(expected, group.column, update.row),
                update.replacement
            );
        }
    }
    assert_eq!(changed.len(), case.changed);
    for (at, (&old, &new)) in before.iter().zip(expected).enumerate() {
        if !payload.contains(&at) {
            let mask = masks.get(&at).copied().unwrap_or(0);
            assert_eq!(old & !mask, new & !mask, "unselected byte {at}");
        }
    }
    if case.changed == 0 {
        assert_eq!(before, expected);
    }
    // The entire Extra/MR2/catalog, maps, indexes, opaque siblings, cache bytes,
    // untouched padding and unused tails are covered by the global complement.
}

/// Literal final-target union for independent public-reader comparisons.
pub(crate) fn final_groups(case: &Case) -> Vec<Group> {
    assert!(case.expected.is_some());
    literal_final_groups(&case.source.before, &case.groups, &case.assignments)
}
