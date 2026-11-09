use super::*;
use sha2::{Digest, Sha256};

#[test]
fn literal_pairs_match_the_original_raw_and_readonly_observation_pins() {
    for (case, expected) in [
        (
            SourceCase::Number,
            "235a57e709bb65bc6a1d0cc6483c2b204f1d3b78924108ff20e96a6671ba263d",
        ),
        (
            SourceCase::Mixed,
            "7604ae8af88cbcaf1be537e03cfc65c6f8095ed20a8f4c1113359c7d176ddfdc",
        ),
    ] {
        let [before, after] = pair(case);
        assert_eq!(before.len(), after.len());
        let actual = Sha256::digest(&after)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    }
}
