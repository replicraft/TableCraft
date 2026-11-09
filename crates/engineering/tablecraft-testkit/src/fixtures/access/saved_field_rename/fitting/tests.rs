//! Cross-language raw-image pins frozen before native candidate construction.
use super::*;
use sha2::{Digest, Sha256};

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn fitting_pairs_match_independent_python_image_predictions() {
    for (case, new_name, before_sha, after_sha) in [
        (
            SourceCase::NumberFirst,
            "TaxAmount",
            "c88bc4cb1090d9b0c5e011010ec76cf9acc33da7ec9d58e67fa873bfaabe18ef",
            "dabf8c77e3f258acb0aa7478c3098652a566def5837e47b76b10cb2f3699d1b2",
        ),
        (
            SourceCase::NumberFirst,
            "X",
            "c88bc4cb1090d9b0c5e011010ec76cf9acc33da7ec9d58e67fa873bfaabe18ef",
            "3c0c3b38664199f8ff505b8b6ce0090a96fed1a53e48d8501fb1727412ee493d",
        ),
        (
            SourceCase::NumberHistory,
            "TaxAmount",
            "6f9b095ddf3d724e34d5af98bea4e8ba88bc2b2bc70ade10814f85c27770c620",
            "8d2153d357f5893b7599adcb90637a1cb81edf386e5e587285365fe16e528772",
        ),
        (
            SourceCase::NumberHistory,
            "X",
            "6f9b095ddf3d724e34d5af98bea4e8ba88bc2b2bc70ade10814f85c27770c620",
            "a0c92322b9e6f2d0ad6247b3b4ce58f64f0646f9b0dde0c6fd285db661050dfc",
        ),
        (
            SourceCase::InlineNumberThenText,
            "TaxAmount",
            "e30ac2923f754892895a97231cee7cc6bd8f880c03f384a21546ed4b7e4b77f3",
            "e5f3215f5a1185b36bf79da23173930890d3786be3d14971f3d3bb59920e7d13",
        ),
        (
            SourceCase::InlineNumberThenText,
            "X",
            "e30ac2923f754892895a97231cee7cc6bd8f880c03f384a21546ed4b7e4b77f3",
            "c783d079c1b937b2c31323186357de6b8f6714c5e20fcabca573671f6c2223e7",
        ),
        (
            SourceCase::TextNumberText,
            "TaxAmount",
            "c28d3c1bf1de4e75f894ac80ae332aa2a5b4b8c3c9111700d989222b8d6267c3",
            "cb9b3b1dc2a345348e38b08559f3d72cbc2c2949e49bec8609e7c645a2aab8f2",
        ),
        (
            SourceCase::TextNumberText,
            "X",
            "c28d3c1bf1de4e75f894ac80ae332aa2a5b4b8c3c9111700d989222b8d6267c3",
            "7cdec766b130d14bc3b4d395d7caf9366c71552cb2b40fb26f3ae92dfef7588e",
        ),
    ] {
        let [before, after] = pair(case, new_name);
        assert_eq!(digest(&before), before_sha);
        assert_eq!(digest(&after), after_sha);
        assert_eq!(before.len(), after.len());
    }
}
