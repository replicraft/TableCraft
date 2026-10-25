use super::*;
use std::ops::Range;

const PROFILE_OFFSET: usize = 20;
const ENGINE_SIGNATURE: Range<usize> = 4..20;

fn database(profile: u8) -> FileCopy {
    let mut bytes = fixture::database();
    bytes[PROFILE_OFFSET] = profile;
    if profile != 1 {
        bytes[ENGINE_SIGNATURE].copy_from_slice(b"Standard ACE DB\0");
    }
    FileCopy::new(bytes)
}

#[test]
fn derived_text_keys_group_and_filter_with_exact_counts_in_priority_profiles() {
    let options = ["--number-format", "invariant", "--collation", "ordinal"];
    for profile in [1, 2, 5, 6] {
        let file = database(profile);
        for having in [false, true] {
            let predicate = if having { " HAVING Count(*)>1" } else { "" };
            let sql = format!(
                "SELECT K,Count(*) AS N,Sum(Score) AS S \
                 FROM (SELECT CStr(Id Mod 2) AS K,Score FROM Items) Q \
                 GROUP BY K{predicate}"
            );
            let expected = if having {
                &b"K\tN\tS\n\"\"\"1\"\"\"\t2\t40\n"[..]
            } else {
                &b"K\tN\tS\n\"\"\"1\"\"\"\t2\t40\n\"\"\"0\"\"\"\t1\t20\n"[..]
            };
            for declared in [false, true] {
                let mut controls = options.to_vec();
                if declared {
                    controls.push("--declared-arithmetic");
                }
                assert_eq!(success(file.query(&sql, &controls)), expected);
                for batch in ["1", "2"] {
                    let mut streaming = controls.clone();
                    streaming.extend(["--stream", "--batch-rows", batch]);
                    assert_eq!(success(file.query(&sql, &streaming)), expected);
                }
            }
        }
    }
}

#[test]
fn explicit_comparison_policy_controls_membership_and_preserves_first_spelling() {
    let file = database(6);
    let sql = concat!(
        "SELECT K,Count(*) AS N,Sum(Score) AS S FROM ",
        "(SELECT IIf(Id=1,'a',IIf(Id=2,'A ','b')) AS K,Score FROM Items) Q ",
        "GROUP BY K",
    );
    for (policy, expected) in [
        (
            "ordinal",
            &b"K\tN\tS\n\"\"\"a\"\"\"\t1\t10\n\"\"\"A \"\"\"\t1\t20\n\"\"\"b\"\"\"\t1\t30\n"[..],
        ),
        (
            "general-legacy",
            &b"K\tN\tS\n\"\"\"a\"\"\"\t2\t30\n\"\"\"b\"\"\"\t1\t30\n"[..],
        ),
        (
            "general-2010",
            &b"K\tN\tS\n\"\"\"a\"\"\"\t2\t30\n\"\"\"b\"\"\"\t1\t30\n"[..],
        ),
    ] {
        assert_eq!(success(file.query(sql, &["--collation", policy])), expected);
        for batch in ["1", "2"] {
            assert_eq!(
                success(file.query(
                    sql,
                    &["--stream", "--collation", policy, "--batch-rows", batch]
                )),
                expected,
            );
        }
    }
    assert!(failure(file.query(sql, &["--stream"])).is_empty());
}

#[test]
fn text_having_suppression_and_top_preserve_only_completed_later_error_prefixes() {
    let file = database(6);
    let sql = concat!(
        "SELECT TOP 1 K,Sum(10\\((Id-1)*(Id-3))) AS S FROM ",
        "(SELECT Id,IIf(Id=1,'skip',IIf(Id=2,'keep','boom')) AS K FROM Items) Q ",
        "GROUP BY K HAVING K<>'skip'",
    );
    assert!(failure(file.query(sql, &["--collation", "ordinal"])).is_empty());
    assert_eq!(
        failure(file.query(
            sql,
            &["--stream", "--collation", "ordinal", "--batch-rows", "1"]
        )),
        b"K\tS\n\"\"\"keep\"\"\"\t-10\n",
    );
    assert!(
        failure(file.query(
            sql,
            &["--stream", "--collation", "ordinal", "--batch-rows", "2"]
        ))
        .is_empty()
    );
}

#[test]
fn singleton_raw_text_key_observers_do_not_force_scalar_publication_or_comparison() {
    let file = database(6);
    let sql = concat!(
        "SELECT LenB(K) AS B,Count(*) AS N,VarType(K) AS V FROM ",
        "(SELECT ChrW(55296) AS K FROM Items WHERE Id=1) Q GROUP BY K",
    );
    let expected = b"B\tN\tV\n2\t1\t8\n";
    assert_eq!(success(file.query(sql, &[])), expected);
    for batch in ["1", "2"] {
        assert_eq!(
            success(file.query(sql, &["--stream", "--batch-rows", batch])),
            expected
        );
    }

    for invalid in [
        "SELECT K FROM (SELECT ChrW(55296) AS K FROM Items WHERE Id=1) Q GROUP BY K",
        "SELECT Count(*) FROM (SELECT ChrW(55296) AS K FROM Items WHERE Id<=2) Q GROUP BY K",
    ] {
        assert!(failure(file.query(invalid, &[])).is_empty());
        assert!(failure(file.query(invalid, &["--stream"])).is_empty());
    }
}
