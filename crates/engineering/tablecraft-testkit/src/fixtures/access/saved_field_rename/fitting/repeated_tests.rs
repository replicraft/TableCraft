//! Sequential raw transforms retain actual source history rather than reset it.
use super::*;
use sha2::{Digest, Sha256};

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn repeated_names_match_independent_pins_without_normalizing_retired_or_free_bytes() {
    for (case, values, pins) in [
        (
            SourceCase::NumberFirst,
            &[][..],
            [
                "dabf8c77e3f258acb0aa7478c3098652a566def5837e47b76b10cb2f3699d1b2",
                "df4662f1f52700169cba30a06710f80d16ed4314a052466272819b6dcf107a03",
                "1094759115ebd3b15ce08ead2c2e24dffd01ba2b275fb270b8ac751965db023b",
                "732cb91c4ee52e1de1c210c4a9654de4e42eb6025181d44fd8d1de77c165fc0f",
            ],
        ),
        (
            SourceCase::NumberHistory,
            &[][..],
            [
                "8d2153d357f5893b7599adcb90637a1cb81edf386e5e587285365fe16e528772",
                "6463c54df20ca551052b0801dcb3283951d23f0d3cb7b6d89dcf98cbaff35cf0",
                "c5fee7f940ff16bf5f8181357377f8d9275528bf7c600167c16d5394fb6ed222",
                "38846d2933a2686dc582c1478b056721eb6ac918f9ff1e1218df11ae049447fc",
            ],
        ),
        (
            SourceCase::NumberFirst,
            &[Some(42), None, Some(-7)][..],
            [
                "47757edee60d755af3e44e6961b01156d5d816239d001cfca71a03cd1ca5f7f2",
                "14c89b83f6f8a9125060b25cdbba5d3dacc177b983d4caf907f8d8fee2ea65c9",
                "f65388629fa678114aebb3d9f9468afadc2b72be708d7d0d6e7438c810439709",
                "23c993d37619d6b7085efedeb562756be186bb3e23cba159d8b9698043e283c4",
            ],
        ),
    ] {
        let empty = source(case);
        let original = if values.is_empty() {
            empty
        } else {
            long_rows(&empty, values)
        };
        let property_rows = word(&original, PROPERTY * PAGE + ROW_COUNT);
        let original_database = row(&original, PROPERTY, property_rows - 1);
        let mut current = original.to_vec();
        for ((old_name, new_name), pin) in [
            ("Score", "TaxAmount"),
            ("TaxAmount", "X"),
            ("X", "Revenue"),
            ("Revenue", "Score"),
        ]
        .into_iter()
        .zip(pins)
        {
            let after = rename_current(&current, old_name, new_name);
            assert_eq!(digest(&after), pin);
            assert_eq!(&after[8 * PAGE..10 * PAGE], &original[8 * PAGE..10 * PAGE]);
            assert_eq!(word(&after, PROPERTY * PAGE + ROW_COUNT), property_rows);
            let database = row(&after, PROPERTY, property_rows - 1);
            assert_eq!(
                &after[database],
                &original[original_database.start..original_database.end]
            );
            for slot in 0..property_rows - 2 {
                let retired = row(&original, PROPERTY, slot);
                assert_eq!(&after[retired.start..retired.end], &original[retired]);
            }
            assert_eq!(rename_current(&after, new_name, new_name), after);
            current = after;
        }
        assert_ne!(
            current, original,
            "Returning spelling must not erase physical history"
        );
        assert_eq!(
            long(&current, DEFINITION * PAGE + DEFINITION_LENGTH),
            long(&original, DEFINITION * PAGE + DEFINITION_LENGTH)
        );
    }
}
