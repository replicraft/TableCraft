//! Independent normal-ACE positive-event originals and literal complete outcomes.
use super::{canonical_creation, data_page, put16, put32, wide_properties};

const PAGE: usize = 4096;
const ORIGINAL_ROWS: [[i32; 2]; 3] = [[1, 10], [2, 20], [3, 30]];
const NAMESPACE: &str = "http://schemas.microsoft.com/office/accessservices/2009/11/application";
const SEQUENTIAL: &str = "<Statements><Action Name=\"SetField\"><Argument Name=\"Field\">Score</Argument><Argument Name=\"Value\">[Score]+[Old].[Score]</Argument></Action><Action Name=\"SetField\"><Argument Name=\"Field\">Score</Argument><Argument Name=\"Value\">[Score]+1</Argument></Action></Statements>";
const INCREMENT: &str = "<Statements><Action Name=\"SetField\"><Argument Name=\"Field\">Score</Argument><Argument Name=\"Value\">[Score]+1</Argument></Action></Statements>";
const MACRO_ID: &str = "<Statements><Action Name=\"SetField\"><Argument Name=\"Field\">Id</Argument><Argument Name=\"Value\">[Id]+10</Argument></Action></Statements>";
const RESTORE: &str = "<Statements><Action Name=\"SetField\"><Argument Name=\"Field\">Score</Argument><Argument Name=\"Value\">[Old].[Score]</Argument></Action></Statements>";
const DELETE: &str = "<Statements><ConditionalBlock><If><Condition>[Old].[Id]=1</Condition><Statements><Action Name=\"RaiseError\"><Argument Name=\"Number\">23</Argument></Action></Statements></If></ConditionalBlock></Statements>";
const LATE: &str = "<Statements><Action Name=\"SetField\"><Argument Name=\"Field\">Score</Argument><Argument Name=\"Value\">[Score]+1</Argument></Action><ConditionalBlock><If><Condition>[Id]=2</Condition><Statements><Action Name=\"RaiseError\"><Argument Name=\"Number\">17</Argument></Action></Statements></If></ConditionalBlock></Statements>";

/// Frozen recipe identities; none is inferred from a producer result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recipe {
    /// Seeded Current, sequential assignment and frozen Old produce Score28.
    Sequential,
    /// A macro changes Id, which is absent from the user request target.
    MacroOnlyTarget,
    /// A matched equal assignment still enters this explicitly chosen event model.
    UserNoopMacroChanges,
    /// The macro restores Old, so final effective change count is zero.
    RestoreOriginal,
    /// Empty requests compile but evaluate no rows.
    Empty,
    /// Empty macro plus equal user assignment leaves the full image unchanged.
    AllNoop,
    /// BeforeDelete is compiled in its own context and not run on UPDATE.
    DormantDelete,
    /// A later row rejects atomically, so no candidate exists.
    LateRaise,
    /// An applicable after-effect is outside the admitted pure event profile.
    AfterUpdate,
    /// A named macro remains an unsupported effect, not an ignored property.
    Named,
    /// Physical names without definitions remain unresolved.
    NamesOnly,
    /// An unrepresented name forbids treating recognized programs as complete.
    Unrepresented,
    /// Unknown action refuses even with empty requests.
    OpaqueAction,
    /// A retained body carrier makes native coverage unresolved.
    BodyPresent,
    /// An original expected value does not match the source.
    Stale,
    /// Duplicate original live ordinals refuse without a candidate.
    Duplicate,
}

/// Qualification order, shared unchanged by future engine and host tests.
pub const RECIPES: [Recipe; 16] = [
    Recipe::Sequential,
    Recipe::MacroOnlyTarget,
    Recipe::UserNoopMacroChanges,
    Recipe::RestoreOriginal,
    Recipe::Empty,
    Recipe::AllNoop,
    Recipe::DormantDelete,
    Recipe::LateRaise,
    Recipe::AfterUpdate,
    Recipe::Named,
    Recipe::NamesOnly,
    Recipe::Unrepresented,
    Recipe::OpaqueAction,
    Recipe::BodyPresent,
    Recipe::Stale,
    Recipe::Duplicate,
];

/// Original user intent, before any macro result is available.
pub struct Update {
    /// Original snapshot ordinal.
    pub ordinal: usize,
    /// Exact original non-NULL Int32 carrier.
    pub expected: i32,
    /// User-requested Current seed, not the macro's final result.
    pub replacement: i32,
}

/// Complete immutable input and independent full-image expectation.
pub struct Case {
    /// Exact source header selector; native application version is unqualified.
    pub header: u8,
    /// Frozen recipe identity.
    pub recipe: Recipe,
    /// Complete original with actual positive native MR2 or explicit refusal carrier.
    pub before: Vec<u8>,
    /// Complete final image; None is an atomic refusal with no candidate.
    pub expected: Option<Vec<u8>>,
    /// Original user requests against Score.
    pub updates: Vec<Update>,
    /// Literal final rows on success; None corresponds to a refusal.
    pub final_rows: Option<[[i32; 2]; 3]>,
    /// Union of distinct source roots whose final record differs.
    pub changed_rows: usize,
    /// Exact catalog-owned Extra payload retained before and after.
    pub extra: Vec<u8>,
    /// Whether an additional body payload is explicitly present.
    pub body_present: bool,
}

fn xml(event: &str, body: &str) -> String {
    format!("<DataMacro xmlns=\"{NAMESPACE}\" Event=\"{event}\">{body}</DataMacro>")
}

fn extra(recipe: Recipe) -> Vec<u8> {
    let body = match recipe {
        Recipe::Sequential => SEQUENTIAL,
        Recipe::MacroOnlyTarget => MACRO_ID,
        Recipe::RestoreOriginal => RESTORE,
        Recipe::LateRaise => LATE,
        Recipe::AllNoop => "<Statements/>",
        Recipe::OpaqueAction => "<Statements><Action Name=\"FutureAction\"/></Statements>",
        _ => INCREMENT,
    };
    let change = xml("BeforeChange", body);
    match recipe {
        Recipe::NamesOnly => wide_properties::bag(&[wide_properties::names(&["ValidateUpdate"])]),
        Recipe::Unrepresented => wide_properties::bag(&[
            wide_properties::names(&["ValidateUpdate", "UnresolvedName"]),
            wide_properties::group(
                3,
                &[wide_properties::property(
                    0,
                    1,
                    12,
                    &wide_properties::text(&change),
                )],
            ),
        ]),
        Recipe::Named => {
            let named = format!(
                "<DataMacro xmlns=\"{NAMESPACE}\" Name=\"Auxiliary\"><Statements/></DataMacro>"
            );
            wide_properties::bag(&[
                wide_properties::names(&["ValidateUpdate", "Auxiliary"]),
                wide_properties::group(
                    3,
                    &[wide_properties::property(
                        0,
                        1,
                        12,
                        &wide_properties::text(&change),
                    )],
                ),
                wide_properties::group(
                    4,
                    &[wide_properties::property(
                        1,
                        1,
                        12,
                        &wide_properties::text(&named),
                    )],
                ),
            ])
        }
        Recipe::DormantDelete => {
            let delete = xml("BeforeDelete", DELETE);
            wide_properties::event_properties(&[
                ("ValidateUpdate", &change),
                ("DeleteGuard", &delete),
            ])
        }
        Recipe::AfterUpdate => {
            let after = xml("AfterUpdate", "<Statements/>");
            wide_properties::event_properties(&[
                ("ValidateUpdate", &change),
                ("AfterUpdate", &after),
            ])
        }
        _ => wide_properties::event_properties(&[("ValidateUpdate", &change)]),
    }
}

fn records(page: &[u8]) -> Vec<Vec<u8>> {
    let count = usize::from(u16::from_le_bytes(page[12..14].try_into().unwrap()));
    assert_eq!(count, 8);
    (0..count)
        .map(|slot| {
            let position = 14 + 2 * slot;
            let start = usize::from(u16::from_le_bytes(
                page[position..position + 2].try_into().unwrap(),
            ));
            let end = if slot == 0 {
                PAGE
            } else {
                usize::from(u16::from_le_bytes(
                    page[position - 2..position].try_into().unwrap(),
                ))
            };
            assert_eq!(start & 0xe000, 0);
            page[start..end].to_vec()
        })
        .collect()
}

fn inline(payload: &[u8]) -> Vec<u8> {
    let mut value = (0x8000_0000_u32 | u32::try_from(payload.len()).unwrap())
        .to_le_bytes()
        .to_vec();
    value.extend_from_slice(&[0; 8]);
    value.extend_from_slice(payload);
    value
}

fn catalog_user(original: &[u8], extra: &[u8], body: bool) -> Vec<u8> {
    assert_eq!(original.len(), 66);
    assert_eq!(&original[32..42], b"I\0t\0e\0m\0s\0");
    let extra = inline(extra);
    let body = body.then(|| inline(&[0x41, 0]));
    // SDK ordinary variable slots0..4 precede LVAL slots5..8. The fixed
    // catalog column numbers of Lv/LvExtra are6/7, independently of slot order.
    let values: [Option<&[u8]>; 9] = [
        Some(&original[32..42]),
        None,
        None,
        None,
        None,
        None,
        body.as_deref(),
        Some(&extra),
        None,
    ];
    let mut row = original[..32].to_vec();
    let mut starts = [0_u16; 9];
    for (value, start) in values.into_iter().zip(&mut starts) {
        *start = row.len() as u16;
        if let Some(value) = value {
            row.extend_from_slice(value);
        }
    }
    let end = row.len() as u16;
    row.extend_from_slice(&end.to_le_bytes());
    for start in starts.into_iter().rev() {
        row.extend_from_slice(&start.to_le_bytes());
    }
    row.extend_from_slice(&9_u16.to_le_bytes());
    row.extend_from_slice(&[if body.is_some() { 0xdf } else { 0x9f }, 0x60]);
    row
}

fn packed(owner: u32, rows: &[Vec<u8>]) -> Vec<u8> {
    let total = rows.iter().map(Vec::len).sum::<usize>();
    assert!(14 + rows.len() * 2 + total <= PAGE);
    let rows = rows.iter().cloned().map(|row| (0, row)).collect::<Vec<_>>();
    let mut page = data_page(owner, &rows);
    put16(&mut page, 2, (PAGE - 14 - rows.len() * 2 - total) as u16);
    page
}

fn row(values: [i32; 2]) -> Vec<u8> {
    let mut bytes = vec![2, 0];
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.push(3);
    bytes
}

/// Construct from only an existing qualified original owner and literal intent.
pub fn source(header: u8, recipe: Recipe) -> Case {
    assert!(matches!(header, 5 | 6));
    let mut before = canonical_creation::source(canonical_creation::Case::Empty).bytes;
    before[20..24].copy_from_slice(&u32::from(header).to_le_bytes());
    let extra = extra(recipe);
    let body_present = recipe == Recipe::BodyPresent;
    let mut catalog = records(&before[4 * PAGE..5 * PAGE]);
    catalog[7] = catalog_user(&catalog[7], &extra, body_present);
    before[4 * PAGE..5 * PAGE].copy_from_slice(&packed(2, &catalog));
    let data = ORIGINAL_ROWS.into_iter().map(row).collect::<Vec<_>>();
    before[8 * PAGE..9 * PAGE].copy_from_slice(&packed(7, &data));
    put32(&mut before, 7 * PAGE + 16, 3);

    let first = match recipe {
        Recipe::UserNoopMacroChanges | Recipe::AllNoop => 10,
        Recipe::LateRaise => 12,
        _ => 17,
    };
    let mut updates = if matches!(recipe, Recipe::Empty | Recipe::OpaqueAction) {
        vec![]
    } else {
        vec![Update {
            ordinal: 0,
            expected: 10,
            replacement: first,
        }]
    };
    if recipe == Recipe::LateRaise {
        updates.push(Update {
            ordinal: 1,
            expected: 20,
            replacement: 22,
        });
    }
    if recipe == Recipe::Duplicate {
        updates.push(Update {
            ordinal: 0,
            expected: 10,
            replacement: 19,
        });
    }
    if recipe == Recipe::Stale {
        updates[0].expected = 999;
    }
    let mut final_rows = ORIGINAL_ROWS;
    let admitted = match recipe {
        Recipe::Sequential => {
            final_rows[0][1] = 28;
            true
        }
        Recipe::MacroOnlyTarget => {
            final_rows[0] = [11, 17];
            true
        }
        Recipe::UserNoopMacroChanges => {
            final_rows[0][1] = 11;
            true
        }
        Recipe::RestoreOriginal | Recipe::Empty | Recipe::AllNoop => true,
        Recipe::DormantDelete => {
            final_rows[0][1] = 18;
            true
        }
        _ => false,
    };
    let changed_rows = if admitted {
        final_rows
            .iter()
            .zip(ORIGINAL_ROWS)
            .filter(|(after, original)| **after != *original)
            .count()
    } else {
        0
    };
    let expected = admitted.then(|| {
        let mut expected = before.clone();
        for (ordinal, (after, original)) in final_rows.iter().zip(ORIGINAL_ROWS).enumerate() {
            let data = &before[8 * PAGE..9 * PAGE];
            let directory = 14 + 2 * ordinal;
            let start = usize::from(u16::from_le_bytes(
                data[directory..directory + 2].try_into().unwrap(),
            ));
            assert_eq!(&data[start..start + 11], row(original));
            for column in 0..2 {
                if after[column] != original[column] {
                    let offset = 8 * PAGE + start + 2 + column * 4;
                    expected[offset..offset + 4].copy_from_slice(&after[column].to_le_bytes());
                }
            }
        }
        // Only the two actual fixed payload spans may change. All maps,
        // catalog/MR2/body bytes, directory words and masks are immutable.
        for offset in 0..before.len() {
            let selected_payload = (0..3).any(|ordinal| {
                let at = 8 * PAGE + 14 + 2 * ordinal;
                let start = 8 * PAGE
                    + usize::from(u16::from_le_bytes(before[at..at + 2].try_into().unwrap()));
                (start + 2..start + 10).contains(&offset)
            });
            if !selected_payload {
                assert_eq!(expected[offset], before[offset]);
            }
        }
        expected
    });
    Case {
        header,
        recipe,
        before,
        expected,
        updates,
        final_rows: admitted.then_some(final_rows),
        changed_rows,
        extra,
        body_present,
    }
}
