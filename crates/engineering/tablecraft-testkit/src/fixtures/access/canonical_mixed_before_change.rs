//! Independent all-scalar mixed originals and literal I32-only expectations.
use super::{
    canonical_fixed_creation as fixed, canonical_variable_creation as variable, wide_properties,
};
use tablecraft_core::values::Value;

const PAGE: usize = 4096;
const NAMESPACE: &str = "http://schemas.microsoft.com/office/accessservices/2009/11/application";

/// Closed intended contexts; existing logical compiler limitations are separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recipe {
    /// Unaligned ordinary I32 with signed-zero fixed scalar siblings.
    FixedSequential,
    /// Real user change restored by Old, preserving the entire mixed file.
    FixedRestore,
    /// Ordinary manual Id and preserved Unicode/Binary/Decimal siblings.
    VariableManual,
    /// Actual readonly AutoNumber Id is an I32 operand.
    CounterUnicode,
    /// NULL and present-empty variable spans remain distinct and unchanged.
    CounterNullEmpty,
    /// Readonly primary AutoNumber Id index remains exact.
    CounterPrimary,
    /// Counter/primary metadata survives a real RestoreOriginal result.
    CounterRestore,
    /// Empty input still observes complete stored programs and metadata.
    CounterEmpty,
    /// A later selected row rejects every proposal.
    LateRaise,
    /// A dormant generated target is outside the intended write profile.
    DormantCounter,
    /// Direct user writes to the generated Id remain refused.
    UserCounter,
    /// Non-I32 Decimal cannot become a fabricated macro frame operand.
    OpaqueOperand,
}

/// Stable source qualification order.
pub const RECIPES: [Recipe; 12] = [
    Recipe::FixedSequential,
    Recipe::FixedRestore,
    Recipe::VariableManual,
    Recipe::CounterUnicode,
    Recipe::CounterNullEmpty,
    Recipe::CounterPrimary,
    Recipe::CounterRestore,
    Recipe::CounterEmpty,
    Recipe::LateRaise,
    Recipe::DormantCounter,
    Recipe::UserCounter,
    Recipe::OpaqueOperand,
];

impl Recipe {
    /// Artifact identity, not inferred from output or values.
    pub const fn id(self) -> &'static str {
        match self {
            Self::FixedSequential => "fixed-sequential",
            Self::FixedRestore => "fixed-restore",
            Self::VariableManual => "variable-manual",
            Self::CounterUnicode => "counter-unicode",
            Self::CounterNullEmpty => "counter-null-empty",
            Self::CounterPrimary => "counter-primary",
            Self::CounterRestore => "counter-restore",
            Self::CounterEmpty => "counter-empty",
            Self::LateRaise => "late-raise",
            Self::DormantCounter => "dormant-counter",
            Self::UserCounter => "user-counter",
            Self::OpaqueOperand => "opaque-operand",
        }
    }
}

/// Original explicit I32 request, before any macro result.
pub struct Update {
    /// Original live-row ordinal.
    pub ordinal: usize,
    /// Original snapshot carrier.
    pub expected: i32,
    /// Requested Current seed.
    pub replacement: i32,
}

/// Intended future normal refusal, not an observed current compiler outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Native generated metadata prevents user or macro targeting.
    GeneratedTarget,
    /// A non-I32 operand is outside the projected expression profile.
    Operand,
    /// Explicit atomic RaiseError number.
    Raised(i32),
}

/// Complete original and independent intended final observation.
pub struct Case {
    /// Header5/6 original, with native-owned explicit BeforeChange MR2.
    pub before: Vec<u8>,
    /// Positive complete expectation; absent for intended refusal contexts.
    pub expected: Option<Vec<u8>>,
    /// Complete decoded ordinary scalar rows, never fake opaque cells.
    pub rows: Vec<Vec<Value>>,
    /// Complete final scalar rows on success.
    pub final_rows: Option<Vec<Vec<Value>>>,
    /// User-selected column spelling.
    pub column: &'static str,
    /// Original requests against that column.
    pub updates: Vec<Update>,
    /// Actual ordinary target native number.
    pub target: usize,
    /// Actual target fixed payload offset excluding row count.
    pub offset: usize,
    /// Actual readonly AutoNumber metadata exists.
    pub counter: bool,
    /// Actual readonly primary Id index exists.
    pub primary: bool,
    /// Exact native-owned MR2 bytes.
    pub extra: Vec<u8>,
    /// Intended future normal refusal; not current all-I32 compiler admission.
    pub refusal: Option<Refusal>,
    /// Final distinct-root union on success.
    pub changed_rows: usize,
}

fn put16(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

fn put32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn records(page: &[u8]) -> Vec<Vec<u8>> {
    let count = usize::from(u16::from_le_bytes(page[12..14].try_into().unwrap()));
    (0..count)
        .map(|slot| {
            let at = 14 + 2 * slot;
            let start = usize::from(u16::from_le_bytes(page[at..at + 2].try_into().unwrap()));
            let end = if slot == 0 {
                PAGE
            } else {
                usize::from(u16::from_le_bytes(page[at - 2..at].try_into().unwrap()))
            };
            assert_eq!(start & 0xe000, 0);
            page[start..end].to_vec()
        })
        .collect()
}

fn packed(owner: u32, rows: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = vec![0; PAGE];
    bytes[..2].copy_from_slice(&[1, 1]);
    put32(&mut bytes, 4, owner);
    put16(&mut bytes, 12, rows.len() as u16);
    let mut end = PAGE;
    for (slot, row) in rows.iter().enumerate() {
        end -= row.len();
        bytes[end..end + row.len()].copy_from_slice(row);
        put16(&mut bytes, 14 + 2 * slot, end as u16);
    }
    put16(&mut bytes, 2, (end - 14 - 2 * rows.len()) as u16);
    bytes
}

fn set(field: &str, value: &str) -> String {
    format!(
        "<Action Name=\"SetField\"><Argument Name=\"Field\">{field}</Argument><Argument Name=\"Value\">{value}</Argument></Action>"
    )
}

fn macro_bag(recipe: Recipe) -> Vec<u8> {
    let body = match recipe {
        Recipe::FixedSequential => format!(
            "{}{}",
            set("Int32Value", "[Int32Value]+[Old].[Int32Value]"),
            set("Int32Value", "[Int32Value]+1")
        ),
        Recipe::FixedRestore => set("Int32Value", "[Old].[Int32Value]"),
        Recipe::CounterRestore => set("Score", "[Old].[Score]"),
        Recipe::DormantCounter => format!(
            "<ConditionalBlock><If><Condition>False</Condition><Statements>{}</Statements></If></ConditionalBlock>",
            set("Id", "1")
        ),
        Recipe::OpaqueOperand => format!(
            "<ConditionalBlock><If><Condition>False</Condition><Statements>{}</Statements></If></ConditionalBlock>",
            set("Score", "[DecimalValue]")
        ),
        _ => {
            let mut body = format!(
                "{}{}",
                set("Score", "[Score]+[Id]"),
                set("Score", "[Score]+[Old].[Score]")
            );
            if recipe == Recipe::LateRaise {
                body.push_str("<ConditionalBlock><If><Condition>[Id]=2</Condition><Statements><Action Name=\"RaiseError\"><Argument Name=\"Number\">17</Argument></Action></Statements></If></ConditionalBlock>");
            }
            body
        }
    };
    let xml = format!(
        "<DataMacro xmlns=\"{NAMESPACE}\" Event=\"BeforeChange\"><Statements>{body}</Statements></DataMacro>"
    );
    wide_properties::event_properties(&[("StoredGuard", &xml)])
}

fn install_extra(image: &mut [u8], extra: &[u8]) {
    let mut rows = records(&image[4 * PAGE..5 * PAGE]);
    assert_eq!(rows.len(), 8);
    let original = &rows[7];
    assert_eq!(original.len(), 66);
    assert_eq!(&original[32..42], b"I\0t\0e\0m\0s\0");
    assert_eq!(&original[64..66], &[0x1f, 0x60]);
    let mut inline = (0x8000_0000_u32 | extra.len() as u32)
        .to_le_bytes()
        .to_vec();
    inline.extend_from_slice(&[0; 8]);
    inline.extend_from_slice(extra);
    let values: [Option<&[u8]>; 9] = [
        Some(&original[32..42]),
        None,
        None,
        None,
        None,
        None,
        None,
        Some(&inline),
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
    row.extend_from_slice(&(row.len() as u16).to_le_bytes());
    for start in starts.into_iter().rev() {
        row.extend_from_slice(&start.to_le_bytes());
    }
    row.extend_from_slice(&9_u16.to_le_bytes());
    row.extend_from_slice(&[0x9f, 0x60]);
    rows[7] = row;
    image[4 * PAGE..5 * PAGE].copy_from_slice(&packed(2, &rows));
}

fn definition(old: &[u8], counter: bool, primary: bool, rows: usize) -> Vec<u8> {
    assert_eq!(&old[45..47], &5_u16.to_le_bytes());
    assert_eq!(&old[43..45], &3_u16.to_le_bytes());
    let old_length = u32::from_le_bytes(old[8..12].try_into().unwrap()) as usize;
    assert_eq!(&old[old_length - 2..old_length], &[0xff, 0xff]);
    let mut active = old[..63].to_vec();
    put16(&mut active, 41, 6);
    put16(&mut active, 45, 6);
    if counter {
        put32(&mut active, 20, rows as u32);
    }
    if primary {
        active.extend_from_slice(&[0; 12]);
        put32(&mut active, 67, rows as u32);
        put32(&mut active, 47, 1);
        put32(&mut active, 51, 1);
    }
    let first = active.len();
    active.extend_from_slice(&old[63..188]);
    assert_eq!(active[first + 15], 3);
    if counter {
        active[first + 15] = 7;
    }
    let mut score = vec![0; 25];
    score[0] = 4;
    put32(&mut score, 1, 1625);
    put16(&mut score, 5, 5);
    put16(&mut score, 7, 3);
    put16(&mut score, 9, 5);
    score[15] = 3;
    put16(&mut score, 21, 4);
    put16(&mut score, 23, 4);
    active.extend_from_slice(&score);
    active.extend_from_slice(&old[188..old_length - 2]);
    active.extend_from_slice(&10_u16.to_le_bytes());
    active.extend_from_slice(b"S\0c\0o\0r\0e\0");
    if primary {
        let mut physical = vec![0; 52];
        put32(&mut physical, 0, 1923);
        for component in 0..10 {
            put16(&mut physical, 4 + 3 * component, 0xffff);
        }
        put16(&mut physical, 4, 0);
        physical[6] = 1;
        put32(&mut physical, 34, (3 << 8) | 16);
        put32(&mut physical, 38, 9);
        physical[46] = 0x89;
        active.extend_from_slice(&physical);
        let mut logical = vec![0; 28];
        put32(&mut logical, 0, 1625);
        put32(&mut logical, 13, u32::MAX);
        logical[23] = 1;
        active.extend_from_slice(&logical);
        let name = "PrimaryId"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        active.extend_from_slice(&(name.len() as u16).to_le_bytes());
        active.extend_from_slice(&name);
    }
    active.extend_from_slice(&[0xff, 0xff]);
    let length = active.len();
    put32(&mut active, 8, length as u32);
    put16(&mut active, 2, (PAGE - length - 8) as u16);
    let mut page = vec![0; PAGE];
    page[..length].copy_from_slice(&active);
    page
}

fn append_score(old: &[u8], score: i32) -> Vec<u8> {
    assert_eq!(&old[..2], &5_u16.to_le_bytes());
    let count_at = old.len() - 3;
    assert_eq!(&old[count_at..count_at + 2], &3_u16.to_le_bytes());
    let footer = old.len() - 11;
    assert_eq!(
        u16::from_le_bytes(old[footer..footer + 2].try_into().unwrap()) as usize,
        footer
    );
    let starts = (0..3)
        .map(|slot| {
            u16::from_le_bytes(
                old[count_at - 2 * (slot + 1)..count_at - 2 * slot]
                    .try_into()
                    .unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(starts[0], 6);
    let mut row = old[..6].to_vec();
    put16(&mut row, 0, 6);
    row.extend_from_slice(&score.to_le_bytes());
    row.extend_from_slice(&old[6..footer]);
    row.extend_from_slice(&((footer + 4) as u16).to_le_bytes());
    for start in starts.into_iter().rev() {
        row.extend_from_slice(&(start + 4).to_le_bytes());
    }
    row.extend_from_slice(&3_u16.to_le_bytes());
    row.push(old[old.len() - 1] | 0x20);
    assert_eq!(row.len(), old.len() + 4);
    assert_eq!(&row[10..footer + 4], &old[6..footer]);
    row
}

fn primary(image: &mut Vec<u8>, rows: usize) {
    image.resize(10 * PAGE, 0);
    let mut maps = records(&image[3 * PAGE..4 * PAGE]);
    assert_eq!(maps.len(), 16);
    let mut map = vec![0; 13];
    map[6] = 2;
    maps.push(map);
    image[3 * PAGE..4 * PAGE].copy_from_slice(&packed(0, &maps));
    let leaf = &mut image[9 * PAGE..10 * PAGE];
    leaf.fill(0xa7);
    leaf[..480].fill(0);
    leaf[..2].copy_from_slice(&[4, 1]);
    put32(leaf, 4, 7);
    put16(leaf, 2, (3616 - rows * 9) as u16);
    for slot in 0..rows {
        let mut key = vec![0x7f];
        key.extend_from_slice(&(((slot + 1) as u32) ^ 0x8000_0000).to_be_bytes());
        key.extend_from_slice(&((8_u32 << 8) | slot as u32).to_be_bytes());
        let end = (slot + 1) * 9;
        leaf[480 + slot * 9..480 + end].copy_from_slice(&key);
        leaf[27 + end / 8] |= 1 << (end % 8);
    }
}

/// Construct fixed/variable cohorts from owned original bytes only.
pub fn source(header: u8, recipe: Recipe) -> Case {
    assert!(matches!(header, 5 | 6));
    let is_fixed = matches!(recipe, Recipe::FixedSequential | Recipe::FixedRestore);
    let counter = !is_fixed && recipe != Recipe::VariableManual;
    let primary_id = !matches!(
        recipe,
        Recipe::FixedSequential
            | Recipe::FixedRestore
            | Recipe::VariableManual
            | Recipe::CounterUnicode
            | Recipe::CounterNullEmpty
    );
    let (mut before, rows, target, offset, column) = if is_fixed {
        let old = fixed::source(fixed::Case::SignedZero);
        (old.bytes, old.rows, 3, 3, "Int32Value")
    } else {
        let old = variable::source(if recipe == Recipe::CounterNullEmpty {
            variable::Case::NullAndEmpty
        } else {
            variable::Case::Unicode
        });
        let mut before = old.bytes;
        let mut rows = old.rows;
        let raw = records(&before[8 * PAGE..9 * PAGE]);
        let raw = raw
            .iter()
            .enumerate()
            .map(|(ordinal, row)| append_score(row, (ordinal as i32 + 1) * 1000))
            .collect::<Vec<_>>();
        for (ordinal, row) in rows.iter_mut().enumerate() {
            row.push(Value::Int32((ordinal as i32 + 1) * 1000));
        }
        let td = definition(&before[7 * PAGE..8 * PAGE], counter, primary_id, rows.len());
        before[7 * PAGE..8 * PAGE].copy_from_slice(&td);
        before[8 * PAGE..9 * PAGE].copy_from_slice(&packed(7, &raw));
        if primary_id {
            primary(&mut before, rows.len());
        }
        (before, rows, 5, 4, "Score")
    };
    before[20..24].copy_from_slice(&u32::from(header).to_le_bytes());
    let extra = macro_bag(recipe);
    install_extra(&mut before, &extra);
    let selected = if recipe == Recipe::CounterNullEmpty {
        1
    } else {
        0
    };
    let original = if is_fixed {
        0
    } else {
        (selected as i32 + 1) * 1000
    };
    let mut updates = vec![Update {
        ordinal: selected,
        expected: original,
        replacement: if is_fixed { 7 } else { 17 },
    }];
    let refusal = match recipe {
        Recipe::DormantCounter | Recipe::UserCounter => Some(Refusal::GeneratedTarget),
        Recipe::OpaqueOperand => Some(Refusal::Operand),
        Recipe::LateRaise => Some(Refusal::Raised(17)),
        _ => None,
    };
    if recipe == Recipe::CounterEmpty {
        updates.clear();
    }
    if recipe == Recipe::LateRaise {
        updates.push(Update {
            ordinal: 1,
            expected: 2000,
            replacement: 19,
        });
    }
    let column = if recipe == Recipe::UserCounter {
        updates = vec![Update {
            ordinal: 0,
            expected: 1,
            replacement: 7,
        }];
        "Id"
    } else {
        column
    };
    let mut final_rows = rows.clone();
    let changed_rows = if refusal.is_some()
        || matches!(
            recipe,
            Recipe::FixedRestore | Recipe::CounterRestore | Recipe::CounterEmpty
        ) {
        0
    } else {
        1
    };
    if changed_rows != 0 {
        final_rows[selected][target] = Value::Int32(if is_fixed {
            8
        } else {
            17 + selected as i32 + 1 + original
        });
    }
    let expected = refusal.is_none().then(|| {
        let mut after = before.clone();
        if changed_rows != 0 {
            let word = 8 * PAGE + 14 + 2 * selected;
            let start = 8 * PAGE
                + usize::from(u16::from_le_bytes(
                    before[word..word + 2].try_into().unwrap(),
                ));
            let patch = start + 2 + offset;
            assert_eq!(&before[patch..patch + 4], &original.to_le_bytes());
            let Value::Int32(final_value) = &final_rows[selected][target] else {
                unreachable!()
            };
            after[patch..patch + 4].copy_from_slice(&final_value.to_le_bytes());
            assert_eq!(&after[..patch], &before[..patch]);
            assert_eq!(&after[patch + 4..], &before[patch + 4..]);
        }
        after
    });
    Case {
        before,
        expected,
        rows,
        final_rows: refusal.is_none().then_some(final_rows),
        column,
        updates,
        target,
        offset,
        counter,
        primary: primary_id,
        extra,
        refusal,
        changed_rows,
    }
}
