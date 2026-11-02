//! Eight independently authored and native/SDK-qualified source images.
use std::{env, fs, path::PathBuf};
use tablecraft_core::values::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Recipe {
    Left,
    Right,
    All,
}

impl Recipe {
    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "retire-left",
            Self::Right => "retire-right",
            Self::All => "delete-all",
        }
    }

    pub fn selected(self) -> std::ops::Range<usize> {
        match self {
            Self::Left => 0..302,
            Self::Right => 302..800,
            Self::All => 0..800,
        }
    }

    pub fn sql(self) -> &'static str {
        match self {
            Self::Left => "DELETE FROM Items WHERE [Id]<=302",
            Self::Right => "DELETE FROM Items WHERE [Id]>302",
            Self::All => "DELETE FROM Items",
        }
    }
}

pub struct Case {
    pub recipe: Recipe,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub definition: usize,
    pub roots: [usize; 2],
    pub left: [usize; 2],
    pub right: [usize; 2],
    pub map_starts: [usize; 2],
}

pub fn load_all(mut verify: impl FnMut(&[u8], &str)) -> Vec<Case> {
    let directory = PathBuf::from(env::var_os("TABLECRAFT_NODE_RETIREMENT_SOURCE_DIR").unwrap());
    assert!(directory.is_absolute());
    let mut result = Vec::new();
    for (header, length, definition, roots, left, right, map_starts, before_hash, after_hashes) in [
        (
            5,
            434176,
            89,
            [91, 92],
            [98, 101],
            [97, 100],
            [372529, 372460],
            "0d9cf91fff6c9153c628bf5cd54dc39a9420b18f4fad669fca8e9a3c8a92694b",
            [
                "2d8d8592a86e4339885fad6ad3222a0f3bbce95cfca99233b12849af212d2b79",
                "007e07984df6d4e49cee492b2f8c793570823e0327f68e94dbdd9429cebef5fe",
                "51a8d030a309648e91945d76144db78c7c8ab574f0d8af7bc579adbae77e16a4",
            ],
        ),
        (
            6,
            503808,
            106,
            [108, 109],
            [115, 118],
            [114, 117],
            [442161, 442092],
            "18eff74333ed55f4bfef879d6a2530fab8031aaef6a07895f50aec13fdf079ae",
            [
                "702425eaf4886321522ac743d8af251b351fa55748524de4f8631cc4149ccbf2",
                "ce808f9787fe418f16f14e317c463ad06c10a9659e623244077751bc99c33951",
                "f1329cc6ce17f1adfb2702f54e1c9a4d02442772e6505b09a92a72c41287230a",
            ],
        ),
    ] {
        let before = fs::read(directory.join(format!("h{header}-node-before.accdb"))).unwrap();
        assert_eq!(before.len(), length);
        verify(&before, before_hash);
        for (recipe, hash) in [Recipe::Left, Recipe::Right, Recipe::All]
            .into_iter()
            .zip(after_hashes)
        {
            let after =
                fs::read(directory.join(format!("h{header}-node-{}.accdb", recipe.label())))
                    .unwrap();
            assert_eq!(after.len(), length);
            verify(&after, hash);
            result.push(Case {
                recipe,
                before: before.clone(),
                after,
                definition,
                roots,
                left,
                right,
                map_starts,
            });
        }
    }
    result
}

pub fn record(ordinal: usize) -> Vec<u8> {
    assert!(ordinal < 800);
    let ordinal = ordinal as i32;
    let mut raw = vec![3, 0];
    for value in [ordinal + 1, ordinal - 400, ordinal % 7] {
        raw.extend_from_slice(&value.to_le_bytes());
    }
    raw.push(7);
    raw
}

pub fn rows(recipe: Option<Recipe>, by_guard: bool) -> Vec<Vec<Value>> {
    let mut ordinals = (0..800)
        .filter(|ordinal| recipe.is_none_or(|recipe| !recipe.selected().contains(ordinal)))
        .collect::<Vec<_>>();
    if by_guard {
        ordinals.sort_unstable_by_key(|ordinal| (ordinal % 7, *ordinal));
    }
    ordinals
        .into_iter()
        .map(|ordinal| {
            let ordinal = ordinal as i32;
            vec![
                Value::Int32(ordinal + 1),
                Value::Int32(ordinal - 400),
                Value::Int32(ordinal % 7),
            ]
        })
        .collect()
}

/// Independent sensitive offsets: complete fences, original ROW suffixes,
/// inactive leaves, history, aliases, original DATA, represented maps and EOF.
pub fn forged_offsets(case: &Case) -> Vec<usize> {
    let mut offsets = vec![
        case.definition * 4096 + 67,
        case.definition * 4096 + 79,
        case.definition * 4096 + 296 + 28 + 12,
        8123 + 5 + case.left[0] / 8,
        case.map_starts[0] + 5,
        case.map_starts[1] + 5,
        case.before.len() - 1,
    ];
    for page in case.roots.into_iter().chain(case.left).chain(case.right) {
        offsets.extend([
            page * 4096,
            page * 4096 + 480,
            page * 4096 + 485,
            page * 4096 + 4095,
        ]);
    }
    // Raw selected DATA is retained even though its logical root is tombstoned.
    let ordinal = case.recipe.selected().start;
    let mut candidates = Vec::new();
    let wanted = record(ordinal);
    for (number, page) in case.before.as_chunks::<4096>().0.iter().enumerate() {
        if page[..2] != [1, 1] || page[4..8] != (case.definition as u32).to_le_bytes() {
            continue;
        }
        for (at, span) in page.windows(15).enumerate() {
            if span == wanted {
                candidates.push(number * 4096 + at);
            }
        }
    }
    assert_eq!(candidates.len(), 1);
    offsets.extend([candidates[0] + 2, candidates[0] + 14]);
    offsets.sort_unstable();
    offsets.dedup();
    offsets
}
