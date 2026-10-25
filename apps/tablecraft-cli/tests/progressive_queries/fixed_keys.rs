use super::*;
use std::ops::Range;

const PROFILE_OFFSET: usize = 20;
const ENGINE_SIGNATURE: Range<usize> = 4..20;

#[test]
fn currency_and_decimal_group_keys_use_exact_values_in_cli_streams_and_having() {
    for profile in [1, 2, 5, 6] {
        let mut bytes = fixture::database();
        bytes[PROFILE_OFFSET] = profile;
        if profile != 1 {
            bytes[ENGINE_SIGNATURE].copy_from_slice(b"Standard ACE DB\0");
        }
        let file = FileCopy::new(bytes);
        for (conversion, first, second) in [("CCur", "1.0000", "0.0000"), ("CDec", "1", "0")] {
            for having in [false, true] {
                let predicate = if having { " HAVING Count(*)>1" } else { "" };
                let sql = format!(
                    "SELECT {conversion}(Id Mod 2) AS K,Count(*) AS N,Sum(Score) AS S \
                     FROM Items GROUP BY {conversion}(Id Mod 2){predicate}"
                );
                let expected = if having {
                    format!("K\tN\tS\n{first}\t2\t40\n")
                } else {
                    format!("K\tN\tS\n{first}\t2\t40\n{second}\t1\t20\n")
                };
                for declared in [false, true] {
                    let mut options = Vec::new();
                    if declared {
                        options.push("--declared-arithmetic");
                    }
                    assert_eq!(success(file.query(&sql, &options)), expected.as_bytes());
                    for batch in ["1", "2"] {
                        let mut streaming = options.clone();
                        streaming.extend(["--stream", "--batch-rows", batch]);
                        assert_eq!(success(file.query(&sql, &streaming)), expected.as_bytes());
                    }
                }
            }
        }
    }
}
