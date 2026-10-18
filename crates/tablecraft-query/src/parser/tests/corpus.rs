//! Original TableCraft examples derived from grammar descriptions, not copied
//! fixtures or observed Access results. Source mapping: docs/SQL_PARSER.md.

use super::*;

const ACCEPTED: &[&str] = &[
    "SELECT * FROM Samples WHERE CreatedAt = #6/12/2025#",
    "SELECT * FROM Samples WHERE Label LIKE 'A*'",
    "SELECT * FROM Samples WHERE Label LIKE 'A%'",
    "SELECT * FROM Samples WHERE NOT Flag",
    "SELECT IIf(Flag, 1, 0) AS Code FROM Samples",
    "SELECT * FROM Samples WHERE Nz(Code, 0) = 1",
    "SELECT * FROM Samples WHERE DateAdd('d', 1, CreatedAt) > 0",
    "SELECT a.Code FROM Samples AS a INNER JOIN Others AS b ON a.Code = b.Code",
    "SELECT * FROM Samples, Others",
    "SELECT * INTO CopyOfSamples FROM Samples",
    "SELECT Label, Count(Code) FROM Samples GROUP BY Label HAVING Count(Code) > 1",
    "SELECT * FROM Samples IN 'external.accdb'",
    "SELECT * FROM Samples WITH OWNERACCESS OPTION",
    "SELECT Label FROM Samples GROUP BY Label",
    "SELECT Label FROM Samples GROUP BY Code + 1",
    "SELECT * FROM Samples ORDER BY Code + 1",
    "SELECT StDev(Code) FROM Samples",
    "SELECT VarP(Code) FROM Samples",
    "SELECT Label FROM Samples GROUP BY Label HAVING Count(Code) > 1",
    "SELECT * FROM (SELECT * FROM Samples) AS s",
    "SELECT TOP 0 * FROM Samples",
    "SELECT TOP 2147483648 * FROM Samples",
    "SELECT \"Code\" FROM Samples",
    "SELECT * FROM Samples WHERE Code = 1.2",
    "SELECT * FROM Samples WHERE Code = 1e2",
    "SELECT * FROM Samples WHERE Code = 9223372036854775808",
    "SELECT * FROM Samples WHERE Code = -9223372036854775809",
    "SELECT * FROM Samples WHERE Code + 1 = 2",
    "SELECT * FROM Samples WHERE Label & 'x' = 'ax'",
    "SELECT * FROM Samples WHERE Label + 'x' = 'ax'",
    "SELECT * FROM Samples WHERE Code IN (1, 2)",
    "SELECT * FROM Samples WHERE Code NOT IN (1, 2)",
    "SELECT * FROM Samples WHERE Code IN (5, 1, 5, NULL)",
    "SELECT * FROM Samples WHERE Label IN ('a,b', \"x\", '')",
    "SELECT * FROM Samples WHERE Code IN (StartCode, EndCode)",
    "SELECT * FROM Samples WHERE Code BETWEEN 1 AND 5",
    "SELECT * FROM Samples WHERE Code BETWEEN 5 AND 1",
    "SELECT * FROM Samples WHERE Code NOT BETWEEN 5 AND 1",
    "SELECT * FROM Samples WHERE Code NOT BETWEEN 1 AND 5",
    "SELECT * FROM Samples WHERE Code BETWEEN 1 AND 1",
    "SELECT * FROM Samples WHERE Code BETWEEN -9223372036854775808 AND 9223372036854775807",
    "SELECT * FROM Samples WHERE Label BETWEEN 'a' AND \"z\"",
    "SELECT * FROM Samples WHERE s.Code BETWEEN s.StartCode AND s.EndCode",
    "SELECT * FROM Samples WHERE Code BETWEEN NULL AND 5",
    "SELECT * FROM Samples;",
    "SELECT [Contact Label]\nFROM [Sample Rows]\nWHERE [Region] = 'West'\nORDER BY [Contact Label]\n;",
    "select [Display Name], Code AS [Public Code] from [Sample Rows]",
    "SELECT s.*, s.Code FROM Samples AS s",
    "SELECT ALL Code FROM Samples",
    "SELECT DISTINCT Code FROM Samples",
    "SELECT DISTINCTROW Code FROM Samples",
    "SELECT TOP 1 Code FROM Samples ORDER BY Code",
    "SELECT TOP 2147483647 * FROM Samples",
    "SELECT TOP 100 PERCENT Code FROM Samples ORDER BY Code DESC, Label ASC",
    "SELECT * FROM Samples WHERE Code = 42",
    "SELECT * FROM Samples WHERE Code <> 42 AND Code < 90",
    "SELECT * FROM Samples WHERE Code <= 42 OR Code > 90",
    "SELECT * FROM Samples WHERE Code >= -9223372036854775808",
    "SELECT * FROM Samples WHERE Code = +9223372036854775807",
    "SELECT * FROM Samples WHERE Label = 'Sam''s sample'",
    "SELECT * FROM Samples WHERE Label = \"West\"",
    "SELECT * FROM Samples WHERE Label = \"Sam's sample\"",
    "SELECT * FROM Samples WHERE Label = \"A \"\"quoted\"\" sample\"",
    "SELECT * FROM Samples WHERE Label IS NULL",
    "SELECT * FROM Samples WHERE Label IS NOT NULL",
    "SELECT * FROM Samples WHERE Flag = TRUE OR Flag = FALSE",
    "SELECT * FROM Samples WHERE Code = NULL",
    "SELECT * FROM Samples WHERE (Code = 1 OR Code = 2) AND Label <> ''",
];

const REJECTED: &[&str] = &[
    // Forms requiring parse_query rather than the single-SELECT entry point.
    "PARAMETERS [Minimum score] Long; SELECT * FROM Samples WHERE Score > [Minimum score]",
    "TRANSFORM Count(Code) SELECT Label FROM Samples GROUP BY Label PIVOT Flag",
    "SELECT * FROM Samples UNION SELECT * FROM Others",
    "SELECT Code FROM Samples UNION ALL SELECT Code FROM Others",
    "SELECT Sum(DISTINCT Code) FROM Samples",
    "INSERT INTO Samples (Code) VALUES (1)",
    "UPDATE Samples SET Code = 1",
    "DELETE FROM Samples",
    // Other dialects must not silently supply Access semantics.
    "SELECT TOP (5) * FROM Samples",
    "SELECT TOP 5 WITH TIES * FROM Samples",
    "SELECT TOP 101 PERCENT * FROM Samples",
    "SELECT TOP 1.5 * FROM Samples",
    "SELECT DISTINCT ON (Code) Code FROM Samples",
    "SELECT * FROM Samples LIMIT 5",
    "SELECT * FROM Samples LIMIT TO 5 ROWS",
    "SELECT * FROM Samples ORDER BY Code NULLS FIRST",
    "SELECT * FROM dbo.Samples",
    "SELECT `Code` FROM Samples",
    "SELECT * FROM Samples WHERE Code = ?",
    "SELECT * FROM Samples WHERE Code = CAST('1' AS INT)",
    "SELECT * FROM Samples WHERE Label || 'x' = 'ax'",
    "SELECT * FROM Samples WHERE CASE WHEN Flag THEN 1 ELSE 0 END = 1",
    "SELECT * FROM Samples WHERE Label = N'abc'",
    "SELECT * FROM Samples WHERE Flag IS TRUE",
    "SELECT * FROM Samples WHERE Code != 1",
    "SELECT * FROM Samples WHERE Code == 1",
    "SELECT * FROM Samples; SELECT * FROM Others",
    "SELECT * FROM Samples;;",
    // Malformed or outside this slice; diagnostics need not call valid Access invalid.
    "",
    "SELECT",
    "SELECT FROM Samples",
    "SELECT Code, FROM Samples",
    "SELECT *",
    "SELECT * FROM",
    "SELECT * FROM Samples WHERE",
    "SELECT * FROM Samples WHERE (Code = 1",
    "SELECT * FROM Samples WHERE Code = 1)",
    "SELECT * FROM Samples ORDER Code",
    "SELECT * FROM Samples ORDER BY",
];

#[test]
fn supported_corpus_round_trips() {
    for sql in ACCEPTED {
        round_trip(sql);
    }
}

#[test]
fn unsupported_and_malformed_corpus_fails_closed() {
    for sql in REJECTED {
        assert!(
            parse_select(sql, ParseLimits::default()).is_err(),
            "accepted: {sql}"
        );
    }
}
