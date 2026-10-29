//! Pattern oracle owns its atoms and recursively enumerates complete raw spans.
//! It imports neither the production parser/frontiers nor dictionary/pruning.
use super::*;
use std::{cell::RefCell, sync::OnceLock};
use tablecraft_expressions::{
    EvalError,
    comparison::{Collation, LikeProfile},
    logic::Truth,
    predicates::{LikeMode, LikeString, like_utf16},
};

#[derive(Clone, Debug)]
enum Atom {
    Many,
    One,
    Digit,
    Literal(Vec<u16>),
    Class(Vec<(u16, u16)>, bool),
}

fn full_key(units: &[u16]) -> Vec<u8> {
    key::encode_full_french(
        units,
        &mut key::Work {
            budget: &mut budget(),
            cancelled: &|| false,
        },
    )
    .unwrap()
    .bytes
}

fn all_single_keys() -> &'static BTreeSet<Vec<u8>> {
    static KEYS: OnceLock<BTreeSet<Vec<u8>>> = OnceLock::new();
    KEYS.get_or_init(|| {
        (u16::MIN..=u16::MAX)
            .map(|unit| full_key(&[unit]))
            .collect()
    })
}

fn normalize(atoms: Vec<Atom>) -> Vec<Atom> {
    let mut result = Vec::new();
    for atom in atoms {
        if let Atom::Literal(next) = atom {
            if next.is_empty() {
                continue;
            }
            if let Some(Atom::Literal(previous)) = result.last_mut() {
                previous.extend(next);
            } else {
                result.push(Atom::Literal(next));
            }
        } else {
            result.push(atom);
        }
    }
    result
}

fn mode_atoms(atoms: &[Atom], mode: LikeMode) -> Vec<Atom> {
    atoms
        .iter()
        .map(|atom| {
            if mode == LikeMode::Ansi92 && matches!(atom, Atom::Digit) {
                Atom::Class(vec![(48, 57)], false)
            } else {
                atom.clone()
            }
        })
        .collect()
}

fn render(atoms: &[Atom], mode: LikeMode) -> Vec<u16> {
    let (many, one, not) = match mode {
        LikeMode::Ansi89 => (b'*', b'?', b'!'),
        LikeMode::Ansi92 => (b'%', b'_', b'^'),
    };
    let mut result = Vec::new();
    for atom in atoms {
        match atom {
            Atom::Many => result.push(u16::from(many)),
            Atom::One => result.push(u16::from(one)),
            Atom::Digit if mode == LikeMode::Ansi89 => result.push(u16::from(b'#')),
            Atom::Digit => result.extend("[0-9]".encode_utf16()),
            Atom::Literal(units) => result.extend(units),
            Atom::Class(ranges, negated) => {
                result.push(u16::from(b'['));
                if *negated {
                    result.push(u16::from(not));
                }
                for &(low, high) in ranges {
                    result.push(low);
                    if low != high {
                        result.extend([u16::from(b'-'), high]);
                    }
                }
                result.push(u16::from(b']'));
            }
        }
    }
    result
}

fn exhaustive(text: &[u16], atoms: &[Atom], at: usize) -> bool {
    let Some((atom, remaining)) = atoms.split_first() else {
        return at == text.len();
    };
    match atom {
        Atom::Many => (at..=text.len()).any(|end| exhaustive(text, remaining, end)),
        Atom::One => at < text.len() && exhaustive(text, remaining, at + 1),
        Atom::Digit => {
            text.get(at).is_some_and(|unit| (48..=57).contains(unit))
                && exhaustive(text, remaining, at + 1)
        }
        Atom::Literal(literal) => {
            let wanted = full_key(literal);
            (at..=text.len())
                .any(|end| full_key(&text[at..end]) == wanted && exhaustive(text, remaining, end))
        }
        Atom::Class(ranges, negated) => {
            let candidates: Vec<_> = (at + 1..=text.len())
                .filter_map(|end| {
                    let key = full_key(&text[at..end]);
                    if !all_single_keys().contains(&key) {
                        return None;
                    }
                    let included = ranges
                        .iter()
                        .any(|&(low, high)| full_key(&[low]) <= key && key <= full_key(&[high]));
                    Some((end, included))
                })
                .collect();
            if *negated && candidates.iter().any(|(_, included)| *included) {
                return false;
            }
            candidates
                .into_iter()
                .any(|(end, included)| included != *negated && exhaustive(text, remaining, end))
        }
    }
}

struct Actual {
    pool: RefCell<ResourceBudget>,
}

impl Collation for Actual {
    fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
        panic!("weighted LIKE must not call the scalar or SQL comparison path")
    }

    fn like_profile(&self) -> LikeProfile {
        LikeProfile::Utf16Weights
    }

    fn compare_like_units(&self, left: &[u16], right: &[u16]) -> Result<Ordering, EvalError> {
        Ok(compare_french_legacy_like_units(
            order(),
            left,
            right,
            &mut self.pool.borrow_mut(),
            &|| false,
        )
        .unwrap())
    }

    fn is_like_unit(&self, units: &[u16]) -> Result<bool, EvalError> {
        Ok(
            is_french_legacy_like_unit(order(), units, &mut self.pool.borrow_mut(), &|| false)
                .unwrap(),
        )
    }

    fn like_literal_step(
        &self,
        candidate: &[u16],
        literal: &[u16],
    ) -> Result<LikeSpanState, EvalError> {
        Ok(french_legacy_like_literal_step(
            order(),
            candidate,
            literal,
            &mut self.pool.borrow_mut(),
            &|| false,
        )
        .unwrap())
    }

    fn like_unit_step(&self, units: &[u16]) -> Result<LikeSpanState, EvalError> {
        Ok(
            french_legacy_like_unit_step(order(), units, &mut self.pool.borrow_mut(), &|| false)
                .unwrap(),
        )
    }
}

fn actual(text: &[u16], atoms: &[Atom], mode: LikeMode) -> bool {
    let text: Vec<_> = text.iter().flat_map(|unit| unit.to_le_bytes()).collect();
    let pattern: Vec<_> = render(atoms, mode)
        .iter()
        .flat_map(|unit| unit.to_le_bytes())
        .collect();
    let provider = Actual {
        pool: RefCell::new(budget()),
    };
    let result = like_utf16(
        LikeString::Utf16Le(&text),
        LikeString::Utf16Le(&pattern),
        mode,
        &provider,
        &mut budget(),
        None,
    )
    .unwrap();
    result == Truth::True
}

#[test]
fn exhaustive_short_patterns_cover_both_grammars_classes_and_excluded_prefixes() {
    let choices = [
        Atom::Many,
        Atom::One,
        Atom::Digit,
        Atom::Literal(vec![97]),
        Atom::Literal(vec![0x00e9]),
        Atom::Literal(vec![0x00df]),
        Atom::Class(vec![(97, 101)], false),
        Atom::Class(vec![(0x00df, 0x00df)], true),
        Atom::Class(vec![(0x00e9, 0x00e8)], false),
    ];
    let alphabet = [97, 115, 0x00df, 0x00e9, 0x00e8, 32, 53];
    let mut texts = vec![vec![]];
    for first in alphabet {
        texts.push(vec![first]);
        for second in alphabet {
            texts.push(vec![first, second]);
        }
    }
    for first in &choices {
        for second in &choices {
            let atoms = normalize(vec![first.clone(), second.clone()]);
            for text in &texts {
                for mode in [LikeMode::Ansi89, LikeMode::Ansi92] {
                    let expected = exhaustive(text, &mode_atoms(&atoms, mode), 0);
                    assert_eq!(
                        actual(text, &atoms, mode),
                        expected,
                        "{text:04x?}/{atoms:?}/{mode:?}"
                    );
                }
            }
        }
    }
}

fn atom() -> impl Strategy<Value = Atom> {
    prop_oneof![
        Just(Atom::Many),
        Just(Atom::One),
        Just(Atom::Digit),
        units().prop_map(Atom::Literal),
        Just(Atom::Class(vec![(0x00e9, 0x00e8)], false)),
        Just(Atom::Class(vec![(0x00df, 0x00df)], true)),
        Just(Atom::Class(vec![(97, 101), (32, 32)], false)),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]
    #[test]
    fn generated_raw_patterns_match_recursive_complete_windows_without_fst_or_pruning(
        text in units(), atoms in prop::collection::vec(atom(), 0..4)
    ) {
        let atoms = normalize(atoms);
        for mode in [LikeMode::Ansi89, LikeMode::Ansi92] {
            let expected = exhaustive(&text, &mode_atoms(&atoms, mode), 0);
            prop_assert_eq!(actual(&text, &atoms, mode), expected, "{:04x?}/{:?}/{:?}", text, atoms, mode);
        }
    }
}
