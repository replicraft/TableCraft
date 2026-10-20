"""Check computed grouping and encounter aggregates against reference rows.

Python derives expected groups/endpoints from read-only Jackcess cursors. This is
an independent row-based model, not an Access SQL engine or optimizer oracle.
"""
import argparse
from pathlib import Path

from compare_browsing import RECIPES, display, run
from compare_large_tables import digest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "database"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    before = digest(args.database)
    controls = 0
    for index in [None, "Number"]:
        cursor = ["--index", "ROULETTE_TB", index] if index else ["ROULETTE_TB"]
        expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                       RECIPES / "TableDataOracle.java", args.database, *cursor)
        rows = [line.split("\t") for line in expected.splitlines()]
        assert len(rows) == 38
        groups = {}
        for row in rows:
            assert row[2].startswith("T")
            text = bytes.fromhex(row[2][1:]).decode("utf-8")
            width = len(text.encode("utf-16-le")) // 2
            groups.setdefault(width, []).append(row)
        options = ["--database", args.database]
        if index:
            options += ["--index", "ROULETTE_TB", index]
        cases = [
            ("SELECT Len([NBR_VALUE]) AS Width,Count(*) AS Total FROM ROULETTE_TB GROUP BY Len([NBR_VALUE]) ORDER BY Width",
             "Width\tTotal\n" + "".join(f"{width}\t{len(group)}\n" for width, group in sorted(groups.items()))),
            ("SELECT First([NBR_VALUE]) AS FirstValue,Last([NBR_VALUE]) AS LastValue FROM ROULETTE_TB",
             "FirstValue\tLastValue\n" + display(rows[0][2]) + "\t" + display(rows[-1][2]) + "\n"),
            ("SELECT Len([NBR_VALUE]) AS Width,First([NBR_VALUE]) AS F,Last([NBR_VALUE]) AS L FROM ROULETTE_TB GROUP BY Len([NBR_VALUE]) ORDER BY Width",
             "Width\tF\tL\n" + "".join(f"{width}\t{display(group[0][2])}\t{display(group[-1][2])}\n" for width, group in sorted(groups.items()))),
            ("SELECT Len([NBR_VALUE])+1 AS Width,Count(*) AS Total FROM ROULETTE_TB GROUP BY Len([NBR_VALUE]) HAVING Len([NBR_VALUE])=2",
             "Width\tTotal\n3\t" + str(len(groups[2])) + "\n"),
        ]
        for sql, result in cases:
            assert run(args.cli, "query", "run", sql, *options) == result, (index, sql)
            controls += 1
    assert digest(args.database) == before
    print(f"Passed {controls} reference-row controls; unchanged source {before}")


if __name__ == "__main__":
    main()
