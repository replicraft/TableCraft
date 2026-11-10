//! Populated name changes are independent of the preserved dense row/key image.
use super::*;
use sha2::{Digest, Sha256};

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn populated_images_match_python_pins_and_commute_with_row_authoring() {
    for (values, name, before_pin, after_pin) in [
        (
            &[Some(42)][..],
            "TaxAmount",
            "3c73d182c8279e3050c056812059de6c8ea26c99d53c40ac9b0e1467ee49d2e6",
            "544da0e202f81fbca77952c7e8851db8c71c0af40f3935d1dd89ad84d395ae14",
        ),
        (
            &[Some(42)][..],
            "X",
            "3c73d182c8279e3050c056812059de6c8ea26c99d53c40ac9b0e1467ee49d2e6",
            "afd3c1f9a66ad014fca113bbc08c2c921ed7b4cdeabaa3d53818d9bd0ba2e762",
        ),
        (
            &[Some(42)][..],
            "Total",
            "3c73d182c8279e3050c056812059de6c8ea26c99d53c40ac9b0e1467ee49d2e6",
            "e6786da4a6b2cb609a761472bc98437016fd329559335b69d0a798e3c86062b5",
        ),
        (
            &[Some(42)][..],
            "Score",
            "3c73d182c8279e3050c056812059de6c8ea26c99d53c40ac9b0e1467ee49d2e6",
            "3c73d182c8279e3050c056812059de6c8ea26c99d53c40ac9b0e1467ee49d2e6",
        ),
        (
            &[Some(42), None, Some(-7)][..],
            "TaxAmount",
            "8e85e6456b412204afb1c21532209108f4300d25ad55151900fdd52c4c006569",
            "47757edee60d755af3e44e6961b01156d5d816239d001cfca71a03cd1ca5f7f2",
        ),
        (
            &[Some(42), None, Some(-7)][..],
            "X",
            "8e85e6456b412204afb1c21532209108f4300d25ad55151900fdd52c4c006569",
            "643a114ba1e3b624a940272564b7aaea0a566ec143e700fd8afd8857da21ff06",
        ),
        (
            &[Some(42), None, Some(-7)][..],
            "Total",
            "8e85e6456b412204afb1c21532209108f4300d25ad55151900fdd52c4c006569",
            "e03d4f4909ee4b16cfd7b09686316f04a24a05cba657a35d24492b0c7b37a079",
        ),
        (
            &[Some(42), None, Some(-7)][..],
            "Score",
            "8e85e6456b412204afb1c21532209108f4300d25ad55151900fdd52c4c006569",
            "8e85e6456b412204afb1c21532209108f4300d25ad55151900fdd52c4c006569",
        ),
        (
            &[None, Some(42)][..],
            "TaxAmount",
            "36a1aaf20409f77a96fc30a58337cb6554573339a8688ae9d6af594b703bc8ab",
            "5a8c4aa0cc8b0b89fa61e246fbe938ef3f09f8971d5b614fe95fa7a4c354ca01",
        ),
        (
            &[None, Some(42)][..],
            "X",
            "36a1aaf20409f77a96fc30a58337cb6554573339a8688ae9d6af594b703bc8ab",
            "77f044e636122c297cc38856b27e015f256fa10137fa1d8a8cce5518c4333e0c",
        ),
        (
            &[None, Some(42)][..],
            "Total",
            "36a1aaf20409f77a96fc30a58337cb6554573339a8688ae9d6af594b703bc8ab",
            "52cabcfbc858f1f44e250b790c6571505a0d53a0391b8b172ebf7e738d47d6e6",
        ),
        (
            &[None, Some(42)][..],
            "Score",
            "36a1aaf20409f77a96fc30a58337cb6554573339a8688ae9d6af594b703bc8ab",
            "36a1aaf20409f77a96fc30a58337cb6554573339a8688ae9d6af594b703bc8ab",
        ),
    ] {
        let [before, after] = populated_number_pair(name, values);
        assert_eq!(digest(&before), before_pin);
        assert_eq!(digest(&after), after_pin);
        let renamed_empty = rename(&source(SourceCase::NumberFirst), "Score", name);
        assert_eq!(after, long_rows(&renamed_empty, values));
        assert_eq!(&before[8 * 4096..10 * 4096], &after[8 * 4096..10 * 4096]);
        for offset in [16, 20, 67] {
            assert_eq!(long(&after, 7 * 4096 + offset), values.len() as u32);
        }
        if name == "Score" {
            assert_eq!(before, after);
        }
    }
}
