"""Check DISTINCTROW against a full-record semijoin model of reference rows.

Read-only Jackcess provides stored values, not Access SQL qualification. Python
computes whole-record equality and output multiplicities independently.
"""
import argparse
from pathlib import Path

from compare_browsing import RECIPES, run
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
        text = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                   RECIPES / "TableDataOracle.java", args.database, *cursor)
        rows = [tuple(line.split("\t")[1:]) for line in text.splitlines()]
        assert len(rows) == 38
        records = list(dict.fromkeys(rows))
        color = lambda row: int(row[2].removeprefix("I32:"))
        number = lambda row: int(row[0].removeprefix("I32:"))
        joined = [(a, b) for a in rows for b in rows if color(a) == color(b)]
        base = " FROM ROULETTE_TB AS A INNER JOIN ROULETTE_TB AS B ON A.COLOR_ID=B.COLOR_ID"
        cases = [
            ("SELECT DISTINCTROW A.COLOR_ID AS C" + base + " ORDER BY C",
             "C", [(color(a),) for a in records]),
            ("SELECT DISTINCT A.COLOR_ID AS C" + base + " ORDER BY C",
             "C", [(c,) for c in set(map(color, records))]),
            ("SELECT DISTINCTROW A.ORDR_NBR AS N" + base + " ORDER BY N",
             "N", [(number(a),) for a in records]),
            ("SELECT DISTINCTROW TOP 5 A.ORDR_NBR AS N" + base + " ORDER BY N",
             "N", [(n,) for n in sorted(map(number, records))[:5]]),
            ("SELECT DISTINCTROW A.COLOR_ID AS C,B.COLOR_ID AS D" + base + " ORDER BY C,D",
             "C\tD", [(color(a), color(b)) for a, b in joined]),
            ("SELECT DISTINCTROW COLOR_ID AS C FROM ROULETTE_TB ORDER BY C",
             "C", [(color(a),) for a in rows]),
            ("SELECT DISTINCTROW A.COLOR_ID AS C" + base + " WHERE B.ORDR_NBR<5 ORDER BY C",
             "C", [(color(a),) for a in records if any(color(a) == color(b) and number(b) < 5 for b in rows)]),
        ]
        options = ["--database", args.database, "--collation", "ordinal"]
        if index:
            options += ["--index", "ROULETTE_TB", index]
        for sql, header, expected in cases:
            expected = header + "\n" + "".join("\t".join(map(str, row)) + "\n" for row in sorted(expected))
            assert run(args.cli, "query", "run", sql, *options) == expected, (index, sql)
            controls += 1
    assert digest(args.database) == before
    print(f"Passed {controls} reference-row controls; unchanged source {before}")


if __name__ == "__main__":
    main()
