"""Check DISTINCTROW against a full-record semijoin model of reference rows.

Read-only Jackcess provides stored values, not Access SQL qualification. Python
computes whole-record equality and output multiplicities independently.
"""
import argparse
from collections import Counter
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
        derived = " FROM (SELECT * FROM ROULETTE_TB) AS A INNER JOIN ROULETTE_TB AS B ON A.COLOR_ID=B.COLOR_ID"
        partial = " FROM (SELECT COLOR_ID FROM ROULETTE_TB) AS A INNER JOIN ROULETTE_TB AS B ON A.COLOR_ID=B.COLOR_ID"
        grouped = " FROM (SELECT COLOR_ID,Count(*) AS Total FROM ROULETTE_TB GROUP BY COLOR_ID) AS A INNER JOIN ROULETTE_TB AS B ON A.COLOR_ID=B.COLOR_ID"
        union = " FROM (SELECT * FROM ROULETTE_TB UNION ALL SELECT * FROM ROULETTE_TB) AS A INNER JOIN ROULETTE_TB AS B ON A.COLOR_ID=B.COLOR_ID"
        computed = " FROM (SELECT ORDR_NBR,COLOR_ID,COLOR_ID+1 AS Hidden FROM ROULETTE_TB) AS A INNER JOIN ROULETTE_TB AS B ON A.COLOR_ID=B.COLOR_ID"
        cases += [
            ("SELECT DISTINCTROW A.COLOR_ID AS C" + derived + " ORDER BY C", "C", [(color(a),) for a in records]),
            ("SELECT DISTINCTROW A.COLOR_ID AS C" + partial + " ORDER BY C", "C", [(c,) for c in set(map(color, rows))]),
            ("SELECT DISTINCTROW A.Total AS N" + grouped + " ORDER BY N", "N", [(count,) for count in Counter(map(color, rows)).values()]),
            ("SELECT DISTINCTROW A.COLOR_ID AS C" + union + " ORDER BY C", "C", [(color(a),) for a in records]),
            ("SELECT DISTINCTROW A.COLOR_ID AS C" + computed + " ORDER BY C", "C", [(color(a),) for a in records]),
            ("SELECT DISTINCTROW A.COLOR_ID AS C,B.COLOR_ID AS D" + partial + " ORDER BY C,D", "C\tD", [(color(a),color(b)) for a,b in joined]),
        ]
        counts = Counter(map(color, rows))
        cases += [
            ("SELECT DISTINCTROW (SELECT A.COLOR_ID) AS C" + base + " ORDER BY C", "C", [(color(a),) for a in records]),
            ("SELECT DISTINCTROW (SELECT (SELECT A.COLOR_ID)) AS C" + base + " ORDER BY C", "C", [(color(a),) for a in records]),
            ("SELECT DISTINCTROW (SELECT Count(*) FROM ROULETTE_TB AS B WHERE B.COLOR_ID=A.COLOR_ID) AS N" + base + " ORDER BY N", "N", [(counts[color(a)],) for a in records]),
            ("SELECT DISTINCTROW (SELECT A.COLOR_ID) AS C,(SELECT B.COLOR_ID) AS D" + base + " ORDER BY C,D", "C\tD", [(color(a),color(b)) for a,b in joined]),
            ("SELECT DISTINCTROW A.COLOR_ID AS C,(SELECT 1 WHERE False) AS N" + base + " ORDER BY C", "C\tN", [(color(a),None) for a in records]),
        ]
        cases += [
            ("SELECT DISTINCTROW 1 AS N" + base, "N", [(1,)] * len(joined)),
            ("SELECT DISTINCTROW (SELECT 1) AS N" + base, "N", [(1,)] * len(joined)),
            ("SELECT DISTINCTROW Count(*) AS N" + base, "N", [(len(joined),)]),
            ("SELECT DISTINCTROW Count(*) AS N" + base + " GROUP BY A.COLOR_ID ORDER BY N", "N", [(count * count,) for count in counts.values()]),
            ("SELECT DISTINCTROW 1 AS N" + base + " GROUP BY A.COLOR_ID", "N", [(1,)] * len(counts)),
        ]
        options = ["--database", args.database, "--collation", "ordinal"]
        if index:
            options += ["--index", "ROULETTE_TB", index]
        for sql, header, expected in cases:
            expected = header + "\n" + "".join("\t".join("NULL" if value is None else str(value) for value in row) + "\n" for row in sorted(expected))
            assert run(args.cli, "query", "run", sql, *options) == expected, (index, sql)
            controls += 1
    assert digest(args.database) == before
    print(f"Passed {controls} reference-row controls; unchanged source {before}")


if __name__ == "__main__":
    main()
