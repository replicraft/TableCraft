use super::*;
use proptest::prelude::*;
use std::cell::Cell;

fn bytes(units: &[u16]) -> ByteString {
    ByteString::from_owned(units.iter().flat_map(|unit| unit.to_le_bytes()).collect()).unwrap()
}

#[test]
fn trimming_retains_isolated_units_and_only_removes_ascii_spaces() {
    for (function, expected) in [
        (TextFunction::LTrim, vec![0xd800, 9, 160, 32]),
        (TextFunction::RTrim, vec![32, 32, 0xd800, 9, 160]),
        (TextFunction::Trim, vec![0xd800, 9, 160]),
    ] {
        let result =
            trim_spaces(function, bytes(&[32, 32, 0xd800, 9, 160, 32]), || Ok(())).unwrap();
        let expected = expected
            .iter()
            .flat_map(|unit| u16::to_le_bytes(*unit))
            .collect::<Vec<_>>();
        assert_eq!(result.as_bytes(), expected);
    }

    for source in [vec![], vec![32], vec![32, 32, 32]] {
        assert!(
            trim_spaces(TextFunction::Trim, bytes(&source), || Ok(()))
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn unsupported_functions_odd_lengths_and_each_edge_checkpoint_refuse_results() {
    assert!(matches!(
        trim_spaces(TextFunction::Len, bytes(&[32]), || panic!(
            "unsupported function scanned input"
        )),
        Err(EvalError::UnsupportedExpression)
    ));
    for function in [TextFunction::LTrim, TextFunction::RTrim, TextFunction::Trim] {
        assert!(matches!(
            trim_spaces(
                function,
                ByteString::from_owned(vec![32, 0, 65]).unwrap(),
                || Ok(())
            ),
            Err(EvalError::UnrepresentableText)
        ));
        let calls = Cell::new(0);
        trim_spaces(function, bytes(&[32, 32, 0xd800, 32, 32]), || {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
        for stop in 1..=calls.get() {
            let visits = Cell::new(0);
            let result = trim_spaces(function, bytes(&[32, 32, 0xd800, 32, 32]), || {
                visits.set(visits.get() + 1);
                if visits.get() >= stop {
                    Err(EvalError::Cancelled)
                } else {
                    Ok(())
                }
            });
            assert!(matches!(result, Err(EvalError::Cancelled)));
        }
    }
}

proptest! {
    #[test]
    fn raw_trim_matches_independent_unit_prefix_and_suffix_removal(
        units in prop::collection::vec(
            prop::sample::select(vec![0, 9, 32, 65, 160, 0x2000, 0xd800, 0xdc00, 0xd83d, 0xde00]),
            0..128,
        ),
    ) {
        for function in [TextFunction::LTrim, TextFunction::RTrim, TextFunction::Trim] {
            let mut expected = units.clone();
            if function != TextFunction::RTrim {
                while expected.first() == Some(&32) {
                    expected.remove(0);
                }
            }
            if function != TextFunction::LTrim {
                while expected.last() == Some(&32) {
                    expected.pop();
                }
            }

            let result = trim_spaces(function, bytes(&units), || Ok(())).unwrap();
            let expected = expected.iter().flat_map(|unit| unit.to_le_bytes()).collect::<Vec<_>>();
            prop_assert_eq!(result.as_bytes(), expected);
        }
    }
}
